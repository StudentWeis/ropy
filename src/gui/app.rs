use crate::clipboard::{self, ClipboardEvent};
use crate::config::{AppTheme, Settings};
use crate::gui::active_window;
use crate::gui::board::RopyBoard;
use crate::gui::tray::{self, TrayEvent};
use crate::repository::{ClipboardRecord, ClipboardRepository};
use gpui::{
    App, AppContext, Application, AsyncApp, Bounds, KeyBinding, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, px, rgb, size,
};
use gpui_component::theme::Theme;
use gpui_component::{Root, ThemeMode};
use std::sync::{
    Arc, Mutex, RwLock,
    mpsc::{self, channel},
};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "macos")]
use objc2::{class, msg_send, runtime::AnyObject};

#[cfg(target_os = "macos")]
fn set_activation_policy_accessory() {
    unsafe {
        // Config the app to be accessory (no dock icon & cmd tab)
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _succeeded: bool = msg_send![app, setActivationPolicy: 1isize];
    }
}

fn initialize_repository() -> Option<Arc<ClipboardRepository>> {
    match ClipboardRepository::new() {
        Ok(repo) => {
            println!("[ropy] Clipboard history repository initialized");
            Some(Arc::new(repo))
        }
        Err(e) => {
            eprintln!("[ropy] Clipboard repository initialization failed: {e}");
            None
        }
    }
}

fn load_initial_records(repository: &Option<Arc<ClipboardRepository>>) -> Vec<ClipboardRecord> {
    repository
        .as_ref()
        .and_then(|repo| repo.get_recent(50).ok())
        .unwrap_or_default()
}

fn start_clipboard_monitor() -> (mpsc::Receiver<ClipboardEvent>, thread::JoinHandle<()>) {
    let (clipboard_tx, clipboard_rx) = channel::<ClipboardEvent>();
    let listener_handle = clipboard::start_clipboard_monitor(clipboard_tx);
    (clipboard_rx, listener_handle)
}

fn start_clipboard_listener(
    clipboard_rx: mpsc::Receiver<ClipboardEvent>,
    shared_records: Arc<Mutex<Vec<ClipboardRecord>>>,
    repository: Option<Arc<ClipboardRepository>>,
    settings: Arc<RwLock<Settings>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while let Ok(event) = clipboard_rx.recv() {
            if let Some(ref repo) = repository {
                let result = match event {
                    ClipboardEvent::Text(text) => repo.save_text(text),
                    ClipboardEvent::Image(path) => repo.save_image_from_path(path),
                };

                match result {
                    Ok(record) => {
                        let mut guard = match shared_records.lock() {
                            Ok(g) => g,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        guard.insert(0, record);
                        let max_history_records = {
                            let settings_guard = settings.read().unwrap();
                            settings_guard.storage.max_history_records
                        };
                        if guard.len() > max_history_records {
                            guard.truncate(max_history_records);
                            repo.cleanup_old_records(max_history_records).ok();
                        }
                    }
                    Err(e) => {
                        eprintln!("[ropy] Failed to save clipboard record: {e}");
                    }
                }
            }
        }
    })
}

fn start_hotkey_monitor() -> (mpsc::Receiver<()>, thread::JoinHandle<()>) {
    let (hotkey_tx, hotkey_rx) = channel();
    let hotkey_handle = crate::gui::hotkey::start_hotkey_listener(move || {
        let _ = hotkey_tx.send(());
    });
    (hotkey_rx, hotkey_handle)
}

fn create_window(
    cx: &mut App,
    shared_records: Arc<Mutex<Vec<ClipboardRecord>>>,
    repository: Option<Arc<ClipboardRepository>>,
    settings: Arc<RwLock<Settings>>,
) -> WindowHandle<Root> {
    let bounds = Bounds::centered(None, size(px(400.), px(600.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::PopUp,
            titlebar: None,
            ..Default::default()
        },
        |window, cx| {
            // Apply the application theme based on settings
            let app_theme = &settings.read().unwrap().theme.get_theme();
            set_app_theme(window, cx, app_theme);

            let view = cx.new(|cx| {
                RopyBoard::new(
                    shared_records,
                    repository.clone(),
                    settings.clone(),
                    window,
                    cx,
                )
            });
            cx.new(|cx| Root::new(view, window, cx))
        },
    )
    .unwrap()
}

/// Set the application theme (light or dark)
pub fn set_app_theme(window: &mut gpui::Window, cx: &mut App, app_theme: &AppTheme) {
    match app_theme.get_theme() {
        AppTheme::Dark => {
            Theme::change(ThemeMode::Dark, Some(window), cx);
            let theme = Theme::global_mut(cx);
            theme.background = rgb(0x2d2d2d).into();
            theme.foreground = rgb(0xffffff).into();
            theme.secondary = rgb(0x3d3d3d).into();
            theme.secondary_foreground = rgb(0xffffff).into();
            theme.border = rgb(0x4d4d4d).into();
            theme.accent = rgb(0x4d4d4d).into();
            theme.muted_foreground = rgb(0x888888).into();
            theme.input = rgb(0x555555).into();
        }
        AppTheme::Light => {
            Theme::change(ThemeMode::Light, Some(window), cx);
            let theme = Theme::global_mut(cx);
            theme.background = rgb(0xffffff).into();
            theme.foreground = rgb(0x1a1a1a).into();
            theme.secondary = rgb(0xf5f5f5).into();
            theme.secondary_foreground = rgb(0x1a1a1a).into();
            theme.border = rgb(0xe0e0e0).into();
            theme.accent = rgb(0xadd8e6).into();
            theme.muted_foreground = rgb(0x6b6b6b).into();
            theme.input = rgb(0xf0f0f0).into();
        }
        _ => {}
    }
}

fn start_hotkey_handler(
    hotkey_rx: mpsc::Receiver<()>,
    window_handle: WindowHandle<Root>,
    async_app: AsyncApp,
) {
    let fg_executor = async_app.foreground_executor().clone();
    let bg_executor = async_app.background_executor().clone();
    fg_executor
        .spawn(async move {
            loop {
                while let Ok(()) = hotkey_rx.try_recv() {
                    let _ = async_app.update(move |cx| {
                        window_handle
                            .update(cx, |_, window, cx| active_window(window, cx))
                            .ok();
                    });
                }
                bg_executor.timer(Duration::from_millis(16)).await;
            }
        })
        .detach();
}

pub fn launch_app() {
    Application::new().run(|cx: &mut App| {
        // Set activation policy on macOS
        #[cfg(target_os = "macos")]
        set_activation_policy_accessory();

        // Initialize gpui-component
        gpui_component::init(cx);

        // Bind global application keys
        bind_application_keys(cx);

        let settings = load_settings();
        let repository = initialize_repository();
        let initial_records = load_initial_records(&repository);
        let shared_records = Arc::new(Mutex::new(initial_records));
        let (clipboard_rx, _listener_handle) = start_clipboard_monitor();
        let (hotkey_rx, _hotkey_handle) = start_hotkey_monitor();
        let _ = start_clipboard_listener(
            clipboard_rx,
            shared_records.clone(),
            repository.clone(),
            settings.clone(),
        );
        let window_handle = create_window(cx, shared_records, repository.clone(), settings.clone());
        let async_app = cx.to_async();
        start_hotkey_handler(hotkey_rx, window_handle, async_app.clone());
        start_tray_handler(window_handle, async_app.clone());

        cx.activate(true);
    });
}

fn bind_application_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", crate::gui::board::Hide, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-q", crate::gui::board::Quit, None),
        #[cfg(target_os = "windows")]
        KeyBinding::new("alt-f4", crate::gui::board::Quit, None),
        KeyBinding::new("up", crate::gui::board::SelectPrev, None),
        KeyBinding::new("down", crate::gui::board::SelectNext, None),
        KeyBinding::new("enter", crate::gui::board::ConfirmSelection, None),
    ]);
}

fn load_settings() -> Arc<RwLock<Settings>> {
    match Settings::load() {
        Ok(s) => {
            println!("[ropy] Settings loaded successfully");
            Arc::new(RwLock::new(s))
        }
        Err(e) => {
            eprintln!("[ropy] Failed to load settings, using defaults: {}", e);
            let default_settings = Settings::default();
            default_settings.save().unwrap_or_else(|err| {
                eprintln!("[ropy] Failed to save default settings: {}", err);
            });
            Arc::new(RwLock::new(default_settings))
        }
    }
}

/// Start the system tray handler
fn start_tray_handler(window_handle: WindowHandle<Root>, async_app: AsyncApp) {
    let fg_executor = async_app.foreground_executor().clone();
    let bg_executor = async_app.background_executor().clone();
    let (tray_tx, tray_rx) = channel::<TrayEvent>();
    match tray::init_tray(tray_tx) {
        Ok(tray) => {
            println!("[ropy] Tray icon initialized successfully");
            // Keep tray icon alive for the lifetime of the application
            Box::leak(Box::new(tray));
            fg_executor
                .spawn(async move {
                    loop {
                        while let Ok(event) = tray_rx.try_recv() {
                            match event {
                                TrayEvent::Show => {
                                    let _ = async_app.update(move |cx| {
                                        window_handle
                                            .update(cx, |_, window, cx| active_window(window, cx))
                                            .ok();
                                    });
                                }
                                TrayEvent::Quit => {
                                    let _ = async_app.update(move |cx| {
                                        cx.quit();
                                    });
                                }
                            }
                        }
                        bg_executor.timer(Duration::from_millis(16)).await;
                    }
                })
                .detach();
        }
        Err(e) => {
            eprintln!("[ropy] Failed to initialize tray icon: {}", e);
        }
    }
}

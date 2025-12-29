use crate::clipboard::{self, ClipboardEvent, LastCopyState};
use crate::config::{AppTheme, AutoStartManager, Settings};
use crate::gui::board::RopyBoard;
use crate::gui::tray::start_tray_handler;
use crate::repository::{ClipboardRecord, ClipboardRepository};
use gpui::{
    App, AppContext, Application, AssetSource, AsyncApp, Bounds, KeyBinding, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, px, rgb, size,
};
use gpui_component::theme::Theme;
use gpui_component::{Root, ThemeMode};
use rust_embed::RustEmbed;
use std::borrow::Cow;
use std::sync::{Arc, Mutex, RwLock};

#[derive(RustEmbed)]
#[folder = "assets"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(|data| data.data))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}

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

fn load_initial_records(
    repository: &Option<Arc<ClipboardRepository>>,
    settings: &Arc<RwLock<Settings>>,
) -> Vec<ClipboardRecord> {
    let max_records = settings.read().unwrap().storage.max_history_records;
    repository
        .as_ref()
        .and_then(|repo| repo.get_recent(max_records).ok())
        .unwrap_or_default()
}

/// Synchronize auto-start state with system on application launch
fn sync_autostart_on_launch(settings: &Arc<RwLock<Settings>>) {
    let autostart_enabled = settings.read().unwrap().autostart.enabled;

    match AutoStartManager::new("Ropy") {
        Ok(manager) => {
            if let Err(e) = manager.sync_state(autostart_enabled) {
                eprintln!("[ropy] Failed to sync auto-start state on launch: {e}");
            } else {
                println!(
                    "[ropy] Auto-start state synced: {}",
                    if autostart_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
            }
        }
        Err(e) => {
            eprintln!("[ropy] Failed to initialize auto-start manager: {e}");
        }
    }
}

fn start_clipboard_monitor(
    async_app: AsyncApp,
    last_copy: Arc<Mutex<LastCopyState>>,
) -> async_channel::Receiver<ClipboardEvent> {
    let (clipboard_tx, clipboard_rx) = async_channel::unbounded::<ClipboardEvent>();
    clipboard::start_clipboard_monitor(clipboard_tx, async_app, last_copy);
    clipboard_rx
}

fn setup_hotkey_listener(
    window_handle: WindowHandle<Root>,
    async_app: AsyncApp,
    settings: Arc<RwLock<Settings>>,
) -> async_channel::Sender<String> {
    let fg_executor = async_app.foreground_executor().clone();
    let bg_executor = async_app.background_executor().clone();
    let hotkey_str = settings.read().unwrap().hotkey.activation_key.clone();
    crate::gui::hotkey::start_hotkey_listener(hotkey_str, fg_executor, bg_executor, move || {
        let _ = async_app.update(move |cx| {
            window_handle
                .update(cx, |_, window, cx| {
                    window.dispatch_action(Box::new(crate::gui::board::Active), cx)
                })
                .ok();
        });
    })
}

fn create_window(
    cx: &mut App,
    shared_records: Arc<Mutex<Vec<ClipboardRecord>>>,
    repository: Option<Arc<ClipboardRepository>>,
    settings: Arc<RwLock<Settings>>,
    last_copy: Arc<Mutex<LastCopyState>>,
    copy_tx: async_channel::Sender<crate::clipboard::CopyRequest>,
    is_silent: bool,
) -> WindowHandle<Root> {
    let bounds = Bounds::centered(None, size(px(400.), px(600.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::PopUp,
            titlebar: None,
            show: !is_silent, // When silent mode, do not show the window initially
            ..Default::default()
        },
        |window, cx| {
            // Apply the application theme based on settings
            let app_theme = &settings.read().unwrap().theme.get_theme();
            set_app_theme(window, cx, app_theme);

            let view = cx.new(|cx| {
                RopyBoard::new(
                    shared_records,
                    repository,
                    settings,
                    last_copy,
                    copy_tx,
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

pub fn launch_app() {
    let args: Vec<String> = std::env::args().collect();
    let is_silent = args.iter().any(|arg| arg == "--silent");

    Application::new().with_assets(Assets).run(move |cx| {
        // Set activation policy on macOS
        #[cfg(target_os = "macos")]
        set_activation_policy_accessory();

        // Initialize gpui-component
        gpui_component::init(cx);

        // Bind global application keys
        bind_application_keys(cx);

        let settings = load_settings();

        // Sync auto-start state on application launch
        sync_autostart_on_launch(&settings);

        let repository = initialize_repository();
        let initial_records = load_initial_records(&repository, &settings);
        let shared_records = Arc::new(Mutex::new(initial_records));
        let last_copy = Arc::new(Mutex::new(LastCopyState::Text("".to_string())));
        let async_app = cx.to_async();
        let clipboard_rx = start_clipboard_monitor(async_app.clone(), last_copy.clone());
        let copy_tx = clipboard::start_clipboard_writer(async_app.clone());
        let window_handle = create_window(
            cx,
            shared_records.clone(),
            repository.clone(),
            settings.clone(),
            last_copy.clone(),
            copy_tx,
            is_silent,
        );
        clipboard::start_clipboard_listener(
            clipboard_rx,
            shared_records,
            repository.clone(),
            settings.clone(),
            async_app.clone(),
            window_handle,
        );
        let hotkey_tx = setup_hotkey_listener(window_handle, async_app.clone(), settings.clone());
        let _ = window_handle.update(cx, |root, _, cx| {
            root.view()
                .clone()
                .downcast::<RopyBoard>()
                .unwrap()
                .update(cx, |board, _| {
                    board.set_hotkey_tx(hotkey_tx);
                });
        });
        start_tray_handler(window_handle, async_app.clone(), settings.clone());

        if !is_silent {
            cx.activate(true);
        }
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
            eprintln!("[ropy] Failed to load settings, using defaults: {e}");
            let default_settings = Settings::default();
            default_settings.save().unwrap_or_else(|err| {
                eprintln!("[ropy] Failed to save default settings: {err}");
            });
            Arc::new(RwLock::new(default_settings))
        }
    }
}

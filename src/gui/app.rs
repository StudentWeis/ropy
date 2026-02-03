use std::{
    borrow::Cow,
    sync::{Arc, Mutex, RwLock},
};

use gpui::{
    App, AppContext, Application, AssetSource, AsyncApp, Bounds, KeyBinding, WindowBounds,
    WindowHandle, WindowKind, WindowOptions, px, rgb, size,
};
use gpui_component::{Root, ThemeMode, theme::Theme};
use rust_embed::RustEmbed;
#[cfg(target_os = "linux")]
use {crate::gui::x11::X11, std::env, std::sync::OnceLock};

use crate::{
    clipboard::{self, ClipboardEvent, LastCopyState},
    config::{AppTheme, AutoStartManager, Settings},
    gui::board::{ConfirmSelection, Hide, Quit, RopyBoard, SelectNext, SelectPrev},
    repository::{ClipboardRecord, ClipboardRepository},
};

#[cfg(target_os = "linux")]
pub static X11: OnceLock<X11> = OnceLock::new();

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

fn initialize_repository() -> Option<Arc<ClipboardRepository>> {
    match ClipboardRepository::new() {
        Ok(repo) => {
            tracing::info!("clipboard history repository initialized");
            Some(Arc::new(repo))
        }
        Err(e) => {
            tracing::error!(error = %e, "clipboard repository initialization failed");
            None
        }
    }
}

fn load_initial_records(
    repository: Option<&Arc<ClipboardRepository>>,
    settings: &Arc<RwLock<Settings>>,
) -> Vec<ClipboardRecord> {
    let max_records = {
        let settings_guard = match settings.read() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        };
        settings_guard.storage.max_history_records
    };
    repository
        .and_then(|repo| repo.get_recent(max_records).ok())
        .unwrap_or_default()
}

/// Synchronize auto-start state with system on application launch
fn sync_autostart_on_launch(settings: &Arc<RwLock<Settings>>) {
    let autostart_enabled = match settings.read() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    }
    .autostart
    .enabled;

    match AutoStartManager::new("Ropy") {
        Ok(manager) => {
            if let Err(e) = manager.sync_state(autostart_enabled) {
                tracing::warn!(error = %e, "failed to sync auto-start state on launch");
            } else {
                tracing::info!(autostart_enabled, "auto-start state synced");
            }
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to initialize auto-start manager");
        }
    }
}

fn start_clipboard_monitor(
    async_app: &AsyncApp,
    last_copy: Arc<Mutex<LastCopyState>>,
) -> async_channel::Receiver<ClipboardEvent> {
    let (clipboard_tx, clipboard_rx) = async_channel::unbounded::<ClipboardEvent>();
    clipboard::start_clipboard_monitor(clipboard_tx, async_app, last_copy);
    clipboard_rx
}

fn setup_hotkey_listener(
    window_handle: WindowHandle<Root>,
    async_app: AsyncApp,
    settings: &Arc<RwLock<Settings>>,
) -> async_channel::Sender<String> {
    let fg_executor = async_app.foreground_executor().clone();
    let bg_executor = async_app.background_executor().clone();
    let hotkey_str = match settings.read() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    }
    .hotkey
    .activation_key
    .clone();
    crate::gui::hotkey::start_hotkey_listener(hotkey_str, &fg_executor, bg_executor, move || {
        let _ = async_app.update(move |cx| {
            window_handle
                .update(cx, |_, window, cx| {
                    window.dispatch_action(Box::new(crate::gui::board::Active), cx);
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
            let app_theme = &match settings.read() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            }
            .theme
            .get_theme();
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
    .unwrap_or_else(|e| {
        tracing::error!(error = %e, "fatal: failed to create window");
        std::process::exit(1);
    })
}

/// Set the application theme (light or dark)
pub fn set_app_theme(window: &mut gpui::Window, cx: &mut App, app_theme: &AppTheme) {
    match app_theme.get_theme() {
        AppTheme::Dark => {
            Theme::change(ThemeMode::Dark, Some(window), cx);
            let theme = Theme::global_mut(cx);
            theme.background = rgb(0x002d_2d2d).into();
            theme.foreground = rgb(0x00ff_ffff).into();
            theme.secondary = rgb(0x003d_3d3d).into();
            theme.secondary_foreground = rgb(0x00ff_ffff).into();
            theme.border = rgb(0x004d_4d4d).into();
            theme.accent = rgb(0x004d_4d4d).into();
            theme.muted_foreground = rgb(0x0088_8888).into();
            theme.input = rgb(0x0055_5555).into();
        }
        AppTheme::Light => {
            Theme::change(ThemeMode::Light, Some(window), cx);
            let theme = Theme::global_mut(cx);
            theme.background = rgb(0x00ff_ffff).into();
            theme.foreground = rgb(0x001a_1a1a).into();
            theme.secondary = rgb(0x00f5_f5f5).into();
            theme.secondary_foreground = rgb(0x001a_1a1a).into();
            theme.border = rgb(0x00e0_e0e0).into();
            theme.accent = rgb(0x00ad_d8e6).into();
            theme.muted_foreground = rgb(0x006b_6b6b).into();
            theme.input = rgb(0x00f0_f0f0).into();
        }
        AppTheme::System => todo!(),
    }
}

pub fn launch_app() {
    let args: Vec<String> = std::env::args().collect();
    let is_silent = args.iter().any(|arg| arg == "--silent");

    Application::new().with_assets(Assets).run(move |cx| {
        // Set activation policy on macOS
        #[cfg(target_os = "macos")]
        super::utils::set_activation_policy_accessory();

        // Initialize gpui-component
        gpui_component::init(cx);

        // Bind global application keys
        bind_application_keys(cx);

        let settings = load_settings();

        // Sync auto-start state on application launch
        sync_autostart_on_launch(&settings);

        let repository = initialize_repository();
        let initial_records = load_initial_records(repository.as_ref(), &settings);
        let shared_records = Arc::new(Mutex::new(initial_records));
        let last_copy = Arc::new(Mutex::new(LastCopyState::Text(String::new())));
        let async_app = cx.to_async();
        let clipboard_rx = start_clipboard_monitor(&async_app, last_copy.clone());
        let copy_tx = clipboard::start_clipboard_writer(&async_app);
        let window_handle = create_window(
            cx,
            shared_records.clone(),
            repository.clone(),
            settings.clone(),
            last_copy,
            copy_tx,
            is_silent,
        );
        clipboard::start_clipboard_listener(
            clipboard_rx,
            shared_records,
            repository,
            settings.clone(),
            async_app.clone(),
            window_handle,
        );
        let hotkey_tx = setup_hotkey_listener(window_handle, async_app.clone(), &settings);
        let _ = window_handle.update(cx, |root, _, cx| {
            if let Ok(board) = root.view().clone().downcast::<RopyBoard>() {
                board.update(cx, |board, _| {
                    board.set_hotkey_tx(hotkey_tx);
                });
            } else {
                tracing::error!("failed to downcast root view to RopyBoard");
            }
        });

        super::tray::start_tray_handler(&settings, async_app, window_handle);

        if !is_silent {
            cx.activate(true);
        }

        // Initialize X11 control
        #[cfg(target_os = "linux")]
        if env::var("DISPLAY").is_ok() {
            let x11 = X11.get_or_init(|| X11::new().expect("Failed to connect x11rb"));
            let _ = x11.active_window();
        }
    });
}

fn bind_application_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Hide, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-q", Quit, None),
        #[cfg(target_os = "windows")]
        KeyBinding::new("alt-f4", Quit, None),
        #[cfg(target_os = "linux")]
        KeyBinding::new("alt-f4", Quit, None),
        KeyBinding::new("up", SelectPrev, None),
        KeyBinding::new("down", SelectNext, None),
        KeyBinding::new("enter", ConfirmSelection, None),
    ]);
}

fn load_settings() -> Arc<RwLock<Settings>> {
    match Settings::load() {
        Ok(s) => {
            tracing::info!("settings loaded successfully");
            Arc::new(RwLock::new(s))
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to load settings; using defaults");
            let default_settings = Settings::default();
            default_settings.save().unwrap_or_else(|err| {
                tracing::error!(error = %err, "failed to save default settings");
            });
            Arc::new(RwLock::new(default_settings))
        }
    }
}

//! Application lifecycle management and subsystem orchestration.
//!
//! This module is the top-level coordinator that wires together the clipboard
//! monitor, repository, GUI, hotkey listener, tray icon, and auto-start
//! subsystems.  It intentionally lives outside `gui` so that the GUI module
//! can focus solely on rendering.

use std::sync::{Arc, Mutex};

use gpui::{App, AppContext, KeyBinding, ReadGlobal, WindowHandle};
use gpui_component::Root;
#[cfg(target_os = "linux")]
use {crate::gui::x11::X11, std::env, std::sync::OnceLock};

use crate::{
    clipboard::{self, ClipboardEvent, LastCopyState},
    config::{AutoStartManager, Settings},
    constants::APP_NAME,
    gui::board::{Active, ConfirmSelection, Hide, Quit, RopyBoard, SelectNext, SelectPrev},
    i18n::I18n,
    repository::{ClipboardRecord, ClipboardRepository},
};

#[cfg(target_os = "linux")]
pub static X11_INSTANCE: OnceLock<X11> = OnceLock::new();

/// Consume clipboard events from the monitor, persist them to the repository,
/// update the in-memory record list, and notify the GUI to refresh.
///
/// This coordinates across three subsystems (clipboard, repository, GUI)
/// and does not belong to the clipboard I/O layer alone.
fn start_clipboard_event_handler(
    clipboard_rx: async_channel::Receiver<ClipboardEvent>,
    shared_records: Arc<Mutex<Vec<ClipboardRecord>>>,
    repository: Option<Arc<ClipboardRepository>>,
    window_handle: WindowHandle<Root>,
    cx: &App,
) {
    let (notify_tx, notify_rx) = async_channel::unbounded::<ClipboardRecord>();
    let bg_repository = repository.clone();

    cx.background_spawn(async move {
        while let Ok(event) = clipboard_rx.recv().await
            && let Some(ref repo) = bg_repository
        {
            let result = match event {
                ClipboardEvent::Text(text) => repo.save_text(text),
                ClipboardEvent::Image(path, hash) => repo.save_image_from_path(path, hash),
            };

            match result {
                Ok(record) => {
                    let _ = notify_tx.send(record).await;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to save clipboard record");
                }
            }
        }
    })
    .detach();

    // Process saved records on the foreground thread where GPUI globals are accessible.
    cx.spawn(async move |async_app| {
        while let Ok(record) = notify_rx.recv().await {
            let _ = async_app.update(|cx| {
                let (max_display, max_storage) = Settings::read(cx, |s| {
                    (s.storage.max_history_records, s.storage.max_storage_records)
                });

                {
                    let mut guard = match shared_records.lock() {
                        Ok(g) => g,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    guard.retain(|r| r.id != record.id);
                    guard.insert(0, record);
                    if guard.len() > max_display {
                        guard.truncate(max_display);
                    }
                }

                if let Some(ref repo) = repository
                    && let Err(e) = repo.cleanup_old_records(max_storage)
                {
                    tracing::warn!(error = %e, "failed to cleanup old clipboard records");
                }

                window_handle
                    .update(cx, |_, _, cx| {
                        cx.notify();
                    })
                    .ok();
            });
        }
    })
    .detach();
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
    max_history_records: usize,
) -> Vec<ClipboardRecord> {
    repository
        .and_then(|repo| repo.get_recent(max_history_records).ok())
        .unwrap_or_default()
}

/// Synchronize auto-start state with system on application launch
fn sync_autostart_on_launch(autostart_enabled: bool) {
    match AutoStartManager::new(APP_NAME) {
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
    cx: &App,
    last_copy: Arc<Mutex<LastCopyState>>,
) -> async_channel::Receiver<ClipboardEvent> {
    let (clipboard_tx, clipboard_rx) = async_channel::unbounded::<ClipboardEvent>();
    clipboard::start_clipboard_monitor(clipboard_tx, cx, last_copy);
    clipboard_rx
}

fn setup_hotkey_listener(
    window_handle: WindowHandle<Root>,
    hotkey_str: String,
    cx: &App,
) -> async_channel::Sender<String> {
    crate::gui::hotkey::start_hotkey_listener(hotkey_str, cx, move |async_app| {
        async_app
            .update(|cx| {
                window_handle
                    .update(cx, |_, window, cx| {
                        window.dispatch_action(Box::new(Active), cx);
                    })
                    .ok();
            })
            .ok();
    })
}

fn bind_application_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("escape", Hide, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-q", Quit, None),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-f4", Quit, None),
        KeyBinding::new("up", SelectPrev, None),
        KeyBinding::new("down", SelectNext, None),
        KeyBinding::new("enter", ConfirmSelection, None),
    ]);
}

fn load_settings() -> Settings {
    match Settings::load() {
        Ok(s) => {
            tracing::info!("settings loaded successfully");
            s
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to load settings; using defaults");
            let default_settings = Settings::default();
            default_settings.save().unwrap_or_else(|err| {
                tracing::error!(error = %err, "failed to save default settings");
            });
            default_settings
        }
    }
}

/// Entry point: initialize all subsystems and launch the application.
pub fn launch() {
    let args: Vec<String> = std::env::args().collect();
    let is_silent = args.iter().any(|arg| arg == "--silent");

    gpui::Application::new()
        .with_assets(crate::gui::Assets)
        .run(move |cx| {
            // Set activation policy on macOS
            #[cfg(target_os = "macos")]
            crate::gui::set_activation_policy_accessory();

            // Initialize gpui-component
            gpui_component::init(cx);

            // Bind global application keys
            bind_application_keys(cx);

            // Register settings as GPUI Global for app-wide access
            let settings = load_settings();
            cx.set_global(settings.clone());

            // Register I18n as GPUI Global for app-wide access
            cx.set_global(I18n::load_i18n(settings.language.clone()));

            // Sync auto-start state on application launch
            sync_autostart_on_launch(settings.autostart.enabled);

            let repository = initialize_repository();
            let initial_records =
                load_initial_records(repository.as_ref(), settings.storage.max_history_records);
            let shared_records = Arc::new(Mutex::new(initial_records));
            let last_copy = Arc::new(Mutex::new(LastCopyState::Text(String::new())));
            let clipboard_rx = start_clipboard_monitor(cx, last_copy.clone());
            let copy_tx = clipboard::start_clipboard_writer(cx);

            let window_handle = crate::gui::create_window(
                cx,
                shared_records.clone(),
                repository.clone(),
                last_copy,
                copy_tx,
                is_silent,
            );
            start_clipboard_event_handler(
                clipboard_rx,
                shared_records,
                repository,
                window_handle,
                cx,
            );
            let hotkey_tx =
                setup_hotkey_listener(window_handle, settings.hotkey.activation_key.clone(), cx);
            // Initialize tray from the global I18n, then pass handles to the board.
            let tray = crate::gui::start_tray_handler(I18n::global(cx), cx, window_handle);

            let board_view = window_handle
                .update(cx, |root, _, _cx| {
                    root.view().clone().downcast::<RopyBoard>().ok()
                })
                .ok()
                .flatten();

            if let Some(board) = &board_view {
                board.update(cx, |board, _| {
                    board.set_hotkey_tx(hotkey_tx);
                    board.set_tray_icon(tray);
                });

                if settings.update.auto_check {
                    board.update(cx, |board, cx| {
                        board.check_for_update_async(cx);
                    });
                }
            } else {
                tracing::error!("failed to downcast root view to RopyBoard");
            }

            if !is_silent {
                cx.activate(true);
            }

            // Initialize X11 control
            #[cfg(target_os = "linux")]
            if env::var("DISPLAY").is_ok() {
                let x11 = X11_INSTANCE.get_or_init(|| X11::new().expect("Failed to connect x11rb"));
                let _ = x11.active_window();
            }
        });
}

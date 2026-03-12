//! Application lifecycle management and subsystem orchestration.
//!
//! This module is the top-level coordinator that wires together the clipboard
//! monitor, repository, GUI, hotkey listener, tray icon, and auto-start
//! subsystems.  It intentionally lives outside `gui` so that the GUI module
//! can focus solely on rendering.

use std::sync::{Arc, Mutex, RwLock};

use gpui::{App, AsyncApp, KeyBinding, WindowHandle};
use gpui_component::Root;
#[cfg(target_os = "linux")]
use {crate::gui::x11::X11, std::env, std::sync::OnceLock};

use crate::{
    clipboard::{self, ClipboardEvent, LastCopyState},
    config::{AutoStartManager, Settings},
    constants::APP_NAME,
    gui::board::{Active, ConfirmSelection, Hide, Quit, RopyBoard, SelectNext, SelectPrev},
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
    settings: Arc<RwLock<Settings>>,
    async_app: AsyncApp,
    window_handle: WindowHandle<Root>,
) {
    let (notify_tx, notify_rx) = async_channel::unbounded::<()>();
    let bg_executor = async_app.background_executor().clone();
    let fg_executor = async_app.foreground_executor().clone();

    bg_executor
        .spawn(async move {
            while let Ok(event) = clipboard_rx.recv().await {
                if let Some(ref repo) = repository {
                    let result = match event {
                        ClipboardEvent::Text(text) => repo.save_text(text),
                        ClipboardEvent::Image(path, hash) => repo.save_image_from_path(path, hash),
                    };

                    match result {
                        Ok(record) => {
                            let (max_display, max_storage) = {
                                let settings_guard = match settings.read() {
                                    Ok(g) => g,
                                    Err(e) => e.into_inner(),
                                };
                                (
                                    settings_guard.storage.max_history_records,
                                    settings_guard.storage.max_storage_records,
                                )
                            };
                            {
                                let mut guard = match shared_records.lock() {
                                    Ok(g) => g,
                                    Err(poisoned) => poisoned.into_inner(),
                                };
                                // Remove existing record with same id (dedup upsert)
                                guard.retain(|r| r.id != record.id);
                                guard.insert(0, record);
                                // Truncate in-memory records to display limit
                                if guard.len() > max_display {
                                    guard.truncate(max_display);
                                }
                            }
                            // Cleanup repository to storage limit
                            if let Err(e) = repo.cleanup_old_records(max_storage) {
                                tracing::warn!(error = %e, "failed to cleanup old clipboard records");
                            }
                            let _ = notify_tx.send(()).await;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "failed to save clipboard record");
                        }
                    }
                }
            }
        })
        .detach();

    // Notify GUI to refresh clipboard history
    fg_executor
        .spawn(async move {
            while (notify_rx.recv().await).is_ok() {
                let _ = async_app.update(|cx| {
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
                    window.dispatch_action(Box::new(Active), cx);
                })
                .ok();
        });
    })
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
            let window_handle = crate::gui::create_window(
                cx,
                shared_records.clone(),
                repository.clone(),
                settings.clone(),
                last_copy,
                copy_tx,
                is_silent,
            );
            start_clipboard_event_handler(
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

                    // Trigger auto-update check on startup if enabled
                    let auto_check = match settings.read() {
                        Ok(g) => g,
                        Err(e) => e.into_inner(),
                    }
                    .update
                    .auto_check;

                    if auto_check {
                        board.update(cx, |board, cx| {
                            board.check_for_update_async(cx);
                        });
                    }
                } else {
                    tracing::error!("failed to downcast root view to RopyBoard");
                }
            });

            crate::gui::start_tray_handler(&settings, async_app, window_handle);

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

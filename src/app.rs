#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Application lifecycle management and subsystem orchestration.
//!
//! This module is the top-level coordinator that wires together the clipboard
//! monitor, repository, GUI, hotkey listener, tray icon, and auto-start
//! subsystems.  It intentionally lives outside `gui` so that the GUI module
//! can focus solely on rendering.

use std::{
    cfg_select,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use gpui_kit::{App, AppContext, KeyBinding, ReadGlobal, WindowHandle, component::Root};
#[cfg(target_os = "linux")]
use {
    crate::gui::x11::X11,
    std::{env, sync::OnceLock},
};

use crate::{
    clipboard::{ClipboardCapture, CopyTracker},
    config::{AutoStartManager, Settings},
    constants::APP_NAME,
    gui::{
        board::{
            Active, ConfirmSelection, ConfirmSelectionPlainText, CycleFilterNext, CycleFilterPrev,
            Hide, Quit, RopyBoard, SelectLeft, SelectNext, SelectPrev, SelectRight,
            ToggleFavoritesFilter,
        },
        repository::GlobalRepository,
        settings::GlobalSettings,
    },
    i18n::I18n,
    repository::ClipboardRepository,
};

#[cfg(target_os = "linux")]
/// Shared X11 connection used for native window mapping and activation.
pub static X11_INSTANCE: OnceLock<X11> = OnceLock::new();

/// Capacity for the clipboard event channel between the OS clipboard listener
/// and the persistence task. Large enough to absorb bursts from apps that copy
/// several times per second, while preventing unbounded memory growth if the
/// consumer falls behind. When full, new events are dropped and logged.
const CLIPBOARD_EVENT_CHANNEL_CAPACITY: usize = 256;

/// Capacity for the UI refresh notification channel. Notifications carry no
/// payload — the foreground task coalesces all pending notifications into a
/// single repository read + UI refresh, so a small bounded capacity is
/// sufficient. When full, additional notifications are no-ops because a
/// refresh is already pending.
const UI_NOTIFY_CHANNEL_CAPACITY: usize = 256;

/// Consume clipboard events from the monitor, persist them to the repository,
/// update the in-memory record list, and notify the GUI to refresh.
///
/// This coordinates across three subsystems (clipboard, repository, GUI)
/// and does not belong to the clipboard I/O layer alone.
///
/// The notification channel is bounded and notifications are coalesced: the
/// foreground task drains all pending `()` notifications before doing a single
/// repository read + UI refresh. This avoids redundant full-list refreshes
/// when many records arrive in rapid succession.
fn start_clipboard_event_handler(
    bg_repository: Arc<ClipboardRepository>,
    clipboard_rx: async_channel::Receiver<ClipboardCapture>,
    window_handle: WindowHandle<Root>,
    cx: &App,
) {
    let (notify_tx, notify_rx) = async_channel::bounded::<()>(UI_NOTIFY_CHANNEL_CAPACITY);

    cx.background_spawn(async move {
        while let Ok(event) = clipboard_rx.recv().await {
            let result = event.persist(&bg_repository);

            match result {
                Ok(_record) => {
                    // Use try_send: if the channel is already full a refresh
                    // is already pending, so dropping this notification is
                    // equivalent to coalescing it with the queued one.
                    match notify_tx.try_send(()) {
                        Ok(()) => {}
                        Err(async_channel::TrySendError::Full(())) => {
                            tracing::debug!("ui refresh notification coalesced (channel full)");
                        }
                        Err(async_channel::TrySendError::Closed(())) => {
                            tracing::warn!(
                                "ui refresh notification channel closed; foreground task gone"
                            );
                        }
                    }
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
        while notify_rx.recv().await.is_ok() {
            // Coalesce: drain all additional pending notifications so a burst
            // of saves results in a single repository read + UI refresh.
            drain_pending_notifications(&notify_rx);

            async_app.update(|cx| {
                window_handle
                    .update(cx, |root, _, cx| {
                        if let Ok(board) = root.view().clone().downcast::<RopyBoard>() {
                            board.update(cx, |board, cx| {
                                board.refresh_records_from_repository(cx);
                                cx.notify();
                            });
                        }
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
    images_dir: PathBuf,
    last_copy: Arc<Mutex<CopyTracker>>,
) -> async_channel::Receiver<ClipboardCapture> {
    let (clipboard_tx, clipboard_rx) =
        async_channel::bounded::<ClipboardCapture>(CLIPBOARD_EVENT_CHANNEL_CAPACITY);
    if let Some((encode, watch)) =
        crate::clipboard::listener::prepare_clipboard_monitor(images_dir, clipboard_tx, last_copy)
    {
        cx.background_spawn(encode).detach();
        cx.background_spawn(async move {
            watch();
        })
        .detach();
    }
    clipboard_rx
}

/// Drain all currently-pending `()` notifications from the channel without
/// blocking. Used for notification coalescing: after one notification has been
/// received, any further notifications already in the queue can be discarded
/// because a single subsequent refresh will reflect all of them.
fn drain_pending_notifications(notify_rx: &async_channel::Receiver<()>) -> usize {
    let mut drained = 0usize;
    while notify_rx.try_recv().is_ok() {
        drained += 1;
    }
    drained
}

fn setup_hotkey_listener(
    window_handle: WindowHandle<Root>,
    hotkey_str: String,
    cx: &App,
) -> async_channel::Sender<crate::gui::hotkey::HotkeyUpdate> {
    crate::gui::hotkey::start_hotkey_listener(hotkey_str, cx, move |async_app| {
        async_app.update(|cx| {
            window_handle
                .update(cx, |_, window, cx| {
                    window.dispatch_action(Box::new(Active), cx);
                })
                .ok();
        });
    })
}

fn bind_application_keys(cx: &mut App) {
    let quit_key_binding = cfg_select! {
        target_os = "macos" => KeyBinding::new("cmd-q", Quit, None),
        _ => KeyBinding::new("alt-f4", Quit, None),
    };

    cx.bind_keys([
        KeyBinding::new("escape", Hide, None),
        quit_key_binding,
        KeyBinding::new("left", SelectLeft, None),
        KeyBinding::new("right", SelectRight, None),
        KeyBinding::new("up", SelectPrev, None),
        KeyBinding::new("down", SelectNext, None),
        KeyBinding::new("enter", ConfirmSelection, None),
        KeyBinding::new("shift-enter", ConfirmSelectionPlainText, None),
        KeyBinding::new("alt-right", CycleFilterNext, None),
        KeyBinding::new("alt-left", CycleFilterPrev, None),
        KeyBinding::new("alt-f", ToggleFavoritesFilter, None),
    ]);
}

fn load_settings() -> Settings {
    match Settings::load() {
        Ok(mut settings) => {
            crate::gui::settings::validate_hotkey(&mut settings);
            tracing::info!("settings loaded successfully");
            settings
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to load settings; using defaults");
            Settings::recovery_defaults()
        }
    }
}

/// Entry point: initialize all subsystems and launch the application.
pub(crate) fn launch() {
    gpui_kit::application()
        .with_assets(crate::gui::Assets)
        .run(move |cx| {
            #[cfg(target_os = "macos")]
            crate::gui::set_activation_policy_accessory();

            gpui_kit::init(cx);
            bind_application_keys(cx);

            // Settings must be installed before the I18n / repository
            // globals because both read from it during their own init.
            let settings = load_settings();
            cx.set_global(GlobalSettings::new(settings.clone()));
            cx.set_global(I18n::load_i18n(settings.language.clone()));
            crate::gui::tray::TrayState::register(cx);

            if !settings.is_recovery_required() {
                sync_autostart_on_launch(settings.autostart.enabled);
            }

            let repository = initialize_repository();
            let repository_ready = repository.is_some();
            cx.set_global(GlobalRepository::new(repository));

            let shared_records = Arc::new(std::sync::RwLock::new(Vec::new()));
            let last_copy = Arc::new(Mutex::new(CopyTracker::default()));
            let (copy_tx, copy_rx) = async_channel::unbounded();
            cx.background_spawn(crate::clipboard::writer::write_clipboard(copy_rx))
                .detach();

            let window_handle =
                crate::gui::create_window(cx, shared_records, last_copy.clone(), copy_tx);
            if let Some(repo) = GlobalRepository::global(cx).cloned() {
                let clipboard_rx =
                    start_clipboard_monitor(cx, repo.images_dir().to_path_buf(), last_copy);
                start_clipboard_event_handler(repo, clipboard_rx, window_handle, cx);
            }
            let hotkey_tx =
                setup_hotkey_listener(window_handle, settings.hotkey.activation_key, cx);
            // Tray initialization needs the loaded I18n; the resulting
            // handle is stashed in the global so menu refreshes after a
            // language change can reach it.
            let tray = crate::gui::start_tray_handler(I18n::global(cx), cx, window_handle);
            crate::gui::tray::TrayState::install(cx, tray);

            let board_view = window_handle
                .update(cx, |root, _, _cx| {
                    root.view().clone().downcast::<RopyBoard>().ok()
                })
                .ok()
                .flatten();

            if let Some(board) = &board_view {
                board.update(cx, |board, _| {
                    board.set_hotkey_tx(hotkey_tx);
                });

                board.update(cx, |board, cx| board.refresh_records_from_repository(cx));
                board.update(cx, RopyBoard::start_update_checks);
                cx.spawn(async move |cx| {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(2))
                        .await;
                    if repository_ready
                        && let Err(error) = crate::updater::transaction::confirm_startup()
                    {
                        tracing::error!(%error, "failed to confirm updated application startup");
                    }
                })
                .detach();
            } else {
                tracing::error!("failed to downcast root view to RopyBoard");
            }

            #[cfg(target_os = "linux")]
            if env::var("DISPLAY").is_ok() {
                match X11::new() {
                    Ok(x11_new) => {
                        let x11 = X11_INSTANCE.get_or_init(|| x11_new);
                        let _ = x11.active_window();
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to connect x11rb; skipping X11 init");
                    }
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::ClipboardEvent;

    #[test]
    fn test_drain_pending_notifications_when_channel_has_queued_items_drains_all() {
        let (tx, rx) = async_channel::bounded::<()>(UI_NOTIFY_CHANNEL_CAPACITY);

        for _ in 0..5 {
            tx.try_send(()).expect("Failed to enqueue notification");
        }

        let drained = drain_pending_notifications(&rx);

        assert_eq!(drained, 5);
        assert!(rx.is_empty());
    }

    #[test]
    fn test_drain_pending_notifications_when_channel_is_empty_returns_zero() {
        let (_tx, rx) = async_channel::bounded::<()>(UI_NOTIFY_CHANNEL_CAPACITY);

        let drained = drain_pending_notifications(&rx);

        assert_eq!(drained, 0);
    }

    #[test]
    fn test_notify_channel_when_full_try_send_returns_full_error() {
        // Simulates the producer path: when the foreground refresh task is
        // briefly stalled, notifications coalesce by being dropped after the
        // bounded channel saturates.
        let (tx, rx) = async_channel::bounded::<()>(UI_NOTIFY_CHANNEL_CAPACITY);

        for _ in 0..UI_NOTIFY_CHANNEL_CAPACITY {
            tx.try_send(()).expect("Failed to enqueue notification");
        }

        let result = tx.try_send(());

        assert!(matches!(result, Err(async_channel::TrySendError::Full(()))));
        assert_eq!(rx.len(), UI_NOTIFY_CHANNEL_CAPACITY);
    }

    #[test]
    fn test_clipboard_event_channel_has_bounded_capacity() {
        let (tx, _rx) = async_channel::bounded::<ClipboardEvent>(CLIPBOARD_EVENT_CHANNEL_CAPACITY);

        for index in 0..CLIPBOARD_EVENT_CHANNEL_CAPACITY {
            tx.try_send(ClipboardEvent::Text(format!("event-{index}")))
                .expect("Failed to enqueue clipboard event");
        }

        let overflow_result = tx.try_send(ClipboardEvent::Text("overflow".to_string()));

        assert!(matches!(
            overflow_result,
            Err(async_channel::TrySendError::Full(_))
        ));
    }
}

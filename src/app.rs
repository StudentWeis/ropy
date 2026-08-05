#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Application lifecycle management and subsystem orchestration.
//!
//! This module is the top-level coordinator that wires together the clipboard
//! monitor, repository, GUI, hotkey listener, tray icon, and auto-start
//! subsystems.  It intentionally lives outside `gui` so that the GUI module
//! can focus solely on rendering.

#[cfg(any(target_os = "linux", test))]
use std::time::Duration;
use std::{
    cfg_select,
    sync::{Arc, Mutex},
};

use gpui::{App, AppContext, KeyBinding, ReadGlobal, WindowHandle};
use gpui_component::Root;
#[cfg(target_os = "linux")]
use {
    crate::gui::x11::X11,
    std::{env, sync::OnceLock},
};

use crate::{
    clipboard::{self, ClipboardEvent, LastCopyState},
    config::{AutoStartManager, Settings},
    constants::APP_NAME,
    gui::board::{
        Active, ConfirmSelection, ConfirmSelectionPlainText, CycleFilterNext, CycleFilterPrev,
        Hide, Quit, RopyBoard, SelectLeft, SelectNext, SelectPrev, SelectRight,
        ToggleFavoritesFilter,
    },
    i18n::I18n,
    repository::{ClipboardRecord, ClipboardRepository, GlobalRepository, backend::StorageBackend},
};

#[cfg(target_os = "linux")]
/// Shared X11 connection used for native window mapping and activation.
pub static X11_INSTANCE: OnceLock<X11> = OnceLock::new();

/// Grace period for GPUI to present the initial Linux frame before X11 unmaps it.
#[cfg(any(target_os = "linux", test))]
const LINUX_STARTUP_HIDE_DELAY: Duration = Duration::from_millis(100);

/// Let GPUI present the initial scene before Linux startup unmaps the window.
///
/// Unmapping synchronously during application setup prevents GPUI's X11
/// renderer from presenting any scene after the window is mapped again. The
/// short foreground delay gives the initial frame time to reach the compositor,
/// preserving tray-resident startup without leaving a transparent surface.
#[cfg(any(target_os = "linux", test))]
fn schedule_linux_window_hide_after_initial_paint(
    window: &gpui::Window,
    cx: &App,
    hide: impl FnOnce() + 'static,
) {
    window
        .spawn(cx, async move |_| {
            gpui::Timer::after(LINUX_STARTUP_HIDE_DELAY).await;
            hide();
        })
        .detach();
}

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
    clipboard_rx: async_channel::Receiver<ClipboardEvent>,
    window_handle: WindowHandle<Root>,
    cx: &App,
) {
    let (notify_tx, notify_rx) = async_channel::bounded::<()>(UI_NOTIFY_CHANNEL_CAPACITY);

    // Clone the Arc before moving into the background task, since GPUI
    // globals are not accessible from background threads.
    let bg_repository = GlobalRepository::global(cx).cloned();

    cx.background_spawn(async move {
        while let Ok(event) = clipboard_rx.recv().await
            && let Some(ref repo) = bg_repository
        {
            let result = match event {
                ClipboardEvent::Text(text) => repo.save_text(text),
                ClipboardEvent::Image(path, hash) => repo.save_image_from_path(path, hash),
                ClipboardEvent::Files(paths) => repo.save_files(&paths),
                ClipboardEvent::RichText {
                    plain_text,
                    html,
                    rtf,
                } => repo.save_rich_text(plain_text, html.as_deref(), rtf.as_deref()),
            };

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

            let _ = async_app.update(|cx| {
                let max_storage = Settings::read(cx, |s| s.storage.max_storage_records);

                GlobalRepository::read(cx, |repo| {
                    if let Some(repo) = repo
                        && let Err(e) = repo.cleanup_old_records_if_needed(max_storage)
                    {
                        tracing::warn!(error = %e, "failed to cleanup old clipboard records");
                    }
                });

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

fn load_initial_records<B: StorageBackend>(
    repository: Option<&Arc<ClipboardRepository<B>>>,
    max_history_records: usize,
) -> Vec<ClipboardRecord> {
    repository
        .and_then(|repo| repo.get_display_records(max_history_records).ok())
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
    let (clipboard_tx, clipboard_rx) =
        async_channel::bounded::<ClipboardEvent>(CLIPBOARD_EVENT_CHANNEL_CAPACITY);
    clipboard::start_clipboard_monitor(clipboard_tx, cx, last_copy);
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
    let quit_key_binding = cfg_select! {
        target_os = "macos" => { KeyBinding::new("cmd-q", Quit, None) },
        _ => { KeyBinding::new("alt-f4", Quit, None) },
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
        Ok(s) => {
            tracing::info!("settings loaded successfully");
            s
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to load settings; using defaults");
            Settings::default()
        }
    }
}

/// Entry point: initialize all subsystems and launch the application.
pub(crate) fn launch() {
    gpui::Application::new()
        .with_assets(crate::gui::Assets)
        .run(move |cx| {
            #[cfg(target_os = "macos")]
            crate::gui::set_activation_policy_accessory();

            gpui_component::init(cx);
            bind_application_keys(cx);

            // Settings must be installed before the I18n / repository
            // globals because both read from it during their own init.
            let settings = load_settings();
            cx.set_global(settings.clone());
            cx.set_global(I18n::load_i18n(settings.language.clone()));
            crate::gui::tray::TrayState::register(cx);

            sync_autostart_on_launch(settings.autostart.enabled);

            let repository = initialize_repository();
            let initial_records =
                load_initial_records(repository.as_ref(), settings.storage.max_history_records);
            cx.set_global(GlobalRepository::new(repository));

            let shared_records = Arc::new(std::sync::RwLock::new(initial_records));
            let last_copy = Arc::new(Mutex::new(LastCopyState::Text(String::new())));
            let clipboard_rx = start_clipboard_monitor(cx, last_copy.clone());
            let copy_tx = clipboard::start_clipboard_writer(cx);

            let window_handle = crate::gui::create_window(cx, shared_records, last_copy, copy_tx);
            start_clipboard_event_handler(clipboard_rx, window_handle, cx);
            let hotkey_tx =
                setup_hotkey_listener(window_handle, settings.hotkey.activation_key.clone(), cx);
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

                if settings.update.auto_check {
                    board.update(cx, |board, cx| {
                        board.check_for_update_async(cx);
                    });
                }
            } else {
                tracing::error!("failed to downcast root view to RopyBoard");
            }

            #[cfg(target_os = "linux")]
            if env::var("DISPLAY").is_ok() {
                match X11::new() {
                    Ok(x11_new) => {
                        X11_INSTANCE.get_or_init(|| x11_new);
                        if let Err(error) = window_handle.update(cx, |_, window, cx| {
                            schedule_linux_window_hide_after_initial_paint(window, cx, || {
                                if let Some(x11) = X11_INSTANCE.get()
                                    && let Err(error) = x11.hide_window()
                                {
                                    tracing::warn!(
                                        error = %error,
                                        "failed to hide Linux window after initial paint"
                                    );
                                }
                            });
                        }) {
                            tracing::warn!(
                                error = %error,
                                "failed to schedule Linux startup window hide"
                            );
                        }
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
    use std::{cell::Cell, rc::Rc, thread};

    use gpui::TestAppContext;

    use super::*;
    use crate::repository::backend::memory::{MemoryBackend, memory_backend_factory};

    #[gpui::test]
    fn test_linux_startup_hide_waits_for_initial_paint_delay(cx: &mut TestAppContext) {
        let hidden = Rc::new(Cell::new(false));
        let hidden_after_frame = hidden.clone();
        let visual_cx = cx.add_empty_window();

        visual_cx.update(|window, cx| {
            schedule_linux_window_hide_after_initial_paint(window, cx, move || {
                hidden_after_frame.set(true);
            });
        });
        visual_cx.run_until_parked();

        assert!(
            !hidden.get(),
            "Linux startup must not unmap the window before the paint delay elapses"
        );
        thread::sleep(LINUX_STARTUP_HIDE_DELAY + Duration::from_millis(50));
        visual_cx.run_until_parked();

        assert!(
            hidden.get(),
            "Linux startup should hide the window after the initial paint delay"
        );
    }

    fn create_test_repo() -> (tempfile::TempDir, ClipboardRepository<MemoryBackend>) {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        let repo = ClipboardRepository::init(
            &db_path,
            temp_dir.path().join("images"),
            memory_backend_factory,
        )
        .expect("Failed to create test repository");

        (temp_dir, repo)
    }

    #[test]
    fn test_load_initial_records_when_repository_is_missing_returns_empty() {
        let records =
            load_initial_records::<crate::repository::backend::redb::RedbBackend>(None, 10);

        assert!(records.is_empty());
    }

    #[test]
    fn test_load_initial_records_when_repository_exists_respects_limit() {
        let (_temp_dir, repo) = create_test_repo();

        repo.save_text("first".to_string())
            .expect("Failed to save first");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("second".to_string())
            .expect("Failed to save second");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("third".to_string())
            .expect("Failed to save third");

        let repo = Arc::new(repo);
        let records = load_initial_records(Some(&repo), 2);

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].content, "third");
        assert_eq!(records[1].content, "second");
    }

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

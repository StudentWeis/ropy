#[cfg(target_os = "linux")]
mod hyprland;
#[cfg(target_os = "linux")]
mod keysym;
#[cfg(target_os = "linux")]
mod portal;

use std::{cell::RefCell, rc::Rc};

use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
#[cfg(target_os = "linux")]
use gpui_kit::BackgroundExecutor;
use gpui_kit::{App, AsyncApp};
#[cfg(target_os = "linux")]
use rust_i18n::t;
use thiserror::Error;

use crate::gui::settings::GlobalSettings;

#[derive(Clone)]
enum ListenerMessage {
    UpdateHotkey(HotkeyUpdate),
    HotkeyEvent(GlobalHotKeyEvent),
    #[cfg(target_os = "linux")]
    PortalActivation {
        id: u32,
        token: Option<String>,
    },
}

#[derive(Debug, Error)]
pub(crate) enum HotkeyUpdateError {
    #[error("global hotkey registration failed")]
    Registration,
    #[error("hotkey settings could not be saved: {0}")]
    Persistence(String),
    #[error("hotkey listener disconnected")]
    Disconnected,
}

#[derive(Clone)]
pub(crate) struct HotkeyUpdate {
    hotkey: String,
    completion: async_channel::Sender<Result<(), HotkeyUpdateError>>,
}

pub(crate) fn request_hotkey_update(
    tx: &async_channel::Sender<HotkeyUpdate>,
    hotkey: String,
) -> Result<async_channel::Receiver<Result<(), HotkeyUpdateError>>, HotkeyUpdateError> {
    let (completion, result) = async_channel::bounded(1);
    tx.try_send(HotkeyUpdate { hotkey, completion })
        .map_err(|_| HotkeyUpdateError::Disconnected)?;
    Ok(result)
}

struct HotkeyListenerState<Manager> {
    current_hotkey: String,
    manager: Option<Manager>,
}

/// Start a global hotkey listener in a foreground task with a custom callback.
///
/// Registers the configured hotkey and invokes the provided callback when the hotkey is pressed.
/// The callback receives an `&AsyncApp` so callers can update the UI without creating their own.
/// Returns a sender to update the hotkey string dynamically.
pub(crate) fn start_hotkey_listener<F>(
    initial_hotkey: String,
    cx: &App,
    on_hotkey: F,
) -> async_channel::Sender<HotkeyUpdate>
where
    F: Fn(&AsyncApp, Option<&str>) + 'static,
{
    let (tx, rx) = async_channel::unbounded::<HotkeyUpdate>();
    let (message_tx, message_rx) = async_channel::unbounded::<ListenerMessage>();

    #[cfg(target_os = "linux")]
    let native_events = !crate::linux::is_wayland();
    #[cfg(not(target_os = "linux"))]
    let native_events = true;
    if native_events {
        spawn_hotkey_event_forwarder(message_tx.clone());
    }
    #[cfg(target_os = "linux")]
    spawn_hotkey_update_forwarder(rx, message_tx.clone());
    #[cfg(not(target_os = "linux"))]
    spawn_hotkey_update_forwarder(rx, message_tx);
    let state = Rc::new(RefCell::new(HotkeyListenerState {
        manager: None,
        current_hotkey: initial_hotkey.clone(),
    }));
    #[cfg(target_os = "linux")]
    let (cancel_tx, cancelled) = async_channel::bounded::<()>(1);
    #[cfg(target_os = "linux")]
    let cleanups = Rc::new(RefCell::new(Vec::<async_channel::Receiver<()>>::new()));
    install_hotkey_shutdown(
        state.clone(),
        message_rx.clone(),
        #[cfg(target_os = "linux")]
        cancel_tx,
        #[cfg(target_os = "linux")]
        cleanups.clone(),
        cx,
    );

    cx.spawn(async move |async_app| {
        #[cfg(target_os = "linux")]
        let manager = register_linux_hotkey(
            initial_hotkey.clone(),
            t!("settings_activation_key").into_owned(),
            cancelled.clone(),
            &cleanups,
            message_tx.clone(),
            async_app.background_executor().clone(),
        )
        .await;
        #[cfg(not(target_os = "linux"))]
        let manager = register_native_hotkey(&initial_hotkey);
        state.borrow_mut().manager = manager;

        while let Ok(message) = message_rx.recv().await {
            if message_rx.is_closed() {
                break;
            }
            let needs_registration = if let ListenerMessage::UpdateHotkey(request) = &message {
                let state = state.borrow();
                !request.hotkey.is_empty()
                    && (!same_hotkey(&request.hotkey, &state.current_hotkey)
                        || state.manager.is_none())
            } else {
                false
            };
            let mut candidate = if let ListenerMessage::UpdateHotkey(request) = &message
                && needs_registration
            {
                #[cfg(target_os = "linux")]
                let registration = register_linux_hotkey(
                    request.hotkey.clone(),
                    t!("settings_activation_key").into_owned(),
                    cancelled.clone(),
                    &cleanups,
                    message_tx.clone(),
                    async_app.background_executor().clone(),
                )
                .await;
                #[cfg(not(target_os = "linux"))]
                let registration = register_native_hotkey(&request.hotkey);
                registration
            } else {
                None
            };
            if message_rx.is_closed() {
                break;
            }
            process_listener_message(
                &mut state.borrow_mut(),
                message,
                &mut |_| candidate.take(),
                &mut |hotkey| {
                    async_app.update(|cx| {
                        let mut settings = GlobalSettings::read(cx, Clone::clone);
                        settings.hotkey.activation_key = hotkey.to_string();
                        settings
                            .save()
                            .map_err(|error| HotkeyUpdateError::Persistence(error.to_string()))?;
                        cx.set_global(GlobalSettings::new(settings));
                        Ok(())
                    })
                },
                &mut |token| on_hotkey(async_app, token),
            );
        }
    })
    .detach();

    tx
}

fn install_hotkey_shutdown(
    state: Rc<RefCell<HotkeyListenerState<Registration>>>,
    messages: async_channel::Receiver<ListenerMessage>,
    #[cfg(target_os = "linux")] cancel_tx: async_channel::Sender<()>,
    #[cfg(target_os = "linux")] cleanups: Rc<RefCell<Vec<async_channel::Receiver<()>>>>,
    cx: &App,
) {
    cx.on_app_quit(move |_| {
        // GPUI has stopped its event loop: take ownership here instead of
        // asking a foreground task to process a shutdown message.
        messages.close();
        let manager = state.borrow_mut().manager.take();
        #[cfg(target_os = "linux")]
        cancel_tx.close();
        #[cfg(target_os = "linux")]
        let pending = std::mem::take(&mut *cleanups.borrow_mut());
        async move {
            drop(manager);
            #[cfg(target_os = "linux")]
            for completion in pending {
                let _ = completion.recv().await;
            }
        }
    })
    .detach();
}

enum Registration {
    #[cfg(target_os = "linux")]
    Portal {
        _manager: portal::Registration,
    },
    Native {
        _manager: GlobalHotKeyManager,
    },
    #[cfg(target_os = "linux")]
    Hyprland {
        _manager: hyprland::Registration,
    },
}

#[cfg(target_os = "linux")]
#[expect(
    clippy::future_not_send,
    reason = "the foreground listener owns the cleanup registry; portal I/O runs in background tasks"
)]
async fn register_linux_hotkey(
    hotkey: String,
    description: String,
    cancelled: async_channel::Receiver<()>,
    cleanups: &RefCell<Vec<async_channel::Receiver<()>>>,
    sender: async_channel::Sender<ListenerMessage>,
    executor: BackgroundExecutor,
) -> Option<Registration> {
    if hotkey.is_empty() {
        return None;
    }
    if crate::linux::hyprland::is_session() {
        return executor
            .spawn(async move { hyprland::Registration::register(&hotkey, sender) })
            .await
            .map(|manager| Registration::Hyprland { _manager: manager });
    }
    if crate::linux::is_wayland() {
        let (completion, completed) = async_channel::bounded(1);
        {
            let mut cleanups = cleanups.borrow_mut();
            cleanups.retain(|cleanup| !cleanup.is_closed());
            cleanups.push(completed);
        }
        let worker_executor = executor.clone();
        return executor
            .spawn(async move {
                portal::Registration::register(
                    hotkey,
                    description,
                    sender,
                    cancelled,
                    completion,
                    worker_executor,
                )
                .await
            })
            .await
            .map(|manager| Registration::Portal { _manager: manager });
    }
    register_native_hotkey(&hotkey)
}

fn register_native_hotkey(hotkey: &str) -> Option<Registration> {
    register_hotkey(hotkey).map(|manager| Registration::Native { _manager: manager })
}

fn process_listener_message<Manager, RegisterHotkey, PersistHotkey, OnHotkey>(
    state: &mut HotkeyListenerState<Manager>,
    message: ListenerMessage,
    register_hotkey: &mut RegisterHotkey,
    persist_hotkey: &mut PersistHotkey,
    on_hotkey: &mut OnHotkey,
) where
    RegisterHotkey: FnMut(&str) -> Option<Manager>,
    PersistHotkey: FnMut(&str) -> Result<(), HotkeyUpdateError>,
    OnHotkey: FnMut(Option<&str>),
{
    match message {
        #[cfg(target_os = "linux")]
        ListenerMessage::PortalActivation { id, token } => {
            if state.manager.is_some()
                && state
                    .current_hotkey
                    .parse::<HotKey>()
                    .is_ok_and(|hotkey| hotkey.id == id)
            {
                on_hotkey(token.as_deref());
            }
        }
        ListenerMessage::UpdateHotkey(request) => {
            let result = update_hotkey(state, &request.hotkey, register_hotkey, persist_hotkey);
            let _ = request.completion.try_send(result);
        }
        ListenerMessage::HotkeyEvent(event) => {
            if event.state() == HotKeyState::Pressed
                && state.manager.is_some()
                && state
                    .current_hotkey
                    .parse::<HotKey>()
                    .is_ok_and(|hotkey| hotkey.id == event.id)
            {
                on_hotkey(None);
            }
        }
    }
}

fn same_hotkey(left: &str, right: &str) -> bool {
    match (left.parse::<HotKey>(), right.parse::<HotKey>()) {
        (Ok(left), Ok(right)) => left == right,
        _ => left == right,
    }
}

fn update_hotkey<Manager>(
    state: &mut HotkeyListenerState<Manager>,
    new_hotkey: &str,
    register: &mut impl FnMut(&str) -> Option<Manager>,
    persist: &mut impl FnMut(&str) -> Result<(), HotkeyUpdateError>,
) -> Result<(), HotkeyUpdateError> {
    if new_hotkey == state.current_hotkey && (state.manager.is_some() || new_hotkey.is_empty()) {
        return Ok(());
    }
    if state.manager.is_some() && same_hotkey(new_hotkey, &state.current_hotkey) {
        persist(new_hotkey)?;
        new_hotkey.clone_into(&mut state.current_hotkey);
        return Ok(());
    }
    // Retain the old registration until both the replacement and its settings
    // are ready. Dropping a rejected candidate unregisters only that candidate.
    let candidate = if new_hotkey.is_empty() {
        None
    } else {
        Some(register(new_hotkey).ok_or(HotkeyUpdateError::Registration)?)
    };
    persist(new_hotkey)?;
    state.manager = candidate;
    state.current_hotkey = new_hotkey.to_string();
    Ok(())
}

fn spawn_hotkey_event_forwarder(message_tx: async_channel::Sender<ListenerMessage>) {
    let receiver = GlobalHotKeyEvent::receiver().clone();
    super::utils::spawn_event_forwarder("hotkey-event-forwarder", message_tx, move |forward| {
        while let Ok(event) = receiver.recv() {
            if !forward(Some(ListenerMessage::HotkeyEvent(event))) {
                break;
            }
        }
    });
}

fn spawn_hotkey_update_forwarder(
    update_rx: async_channel::Receiver<HotkeyUpdate>,
    message_tx: async_channel::Sender<ListenerMessage>,
) {
    super::utils::spawn_event_forwarder("hotkey-update-forwarder", message_tx, move |forward| {
        while let Ok(hotkey) = update_rx.recv_blocking() {
            if !forward(Some(ListenerMessage::UpdateHotkey(hotkey))) {
                break;
            }
        }
    });
}
fn register_hotkey(hotkey_str: &str) -> Option<GlobalHotKeyManager> {
    if hotkey_str.is_empty() {
        return None;
    }
    let hotkey = hotkey_str.parse::<HotKey>().ok()?;
    let manager = GlobalHotKeyManager::new()
        .map_err(|error| {
            tracing::warn!(%error, "failed to create global hotkey manager");
        })
        .ok()?;
    manager
        .register(hotkey)
        .map_err(|error| {
            tracing::warn!(%error, "failed to register global hotkey");
        })
        .ok()?;
    Some(manager)
}

#[cfg(test)]
#[expect(clippy::expect_used)]
mod tests {
    use std::cell::Cell;

    use rstest::rstest;

    use super::*;

    fn update_message(hotkey: &str) -> ListenerMessage {
        let (completion, _result) = async_channel::bounded(1);
        ListenerMessage::UpdateHotkey(HotkeyUpdate {
            hotkey: hotkey.to_string(),
            completion,
        })
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_portal_activation_for_current_shortcut_preserves_token_and_ignores_old_id() {
        let hotkey = "ctrl+shift+a".parse::<HotKey>().expect("hotkey");
        let mut state = HotkeyListenerState {
            current_hotkey: "ctrl+shift+a".into(),
            manager: Some(()),
        };
        let mut tokens = Vec::new();
        for id in [hotkey.id.wrapping_add(1), hotkey.id] {
            process_listener_message(
                &mut state,
                ListenerMessage::PortalActivation {
                    id,
                    token: Some("compositor-token".into()),
                },
                &mut |_| None,
                &mut |_| Ok(()),
                &mut |token| tokens.push(token.map(str::to_owned)),
            );
        }
        assert_eq!(tokens, vec![Some("compositor-token".to_owned())]);
    }

    #[test]
    fn test_hotkey_alias_persistence_failure_keeps_existing_registration() {
        let mut state = HotkeyListenerState {
            current_hotkey: "ctrl+shift+a".into(),
            manager: Some("old"),
        };
        let result = update_hotkey(
            &mut state,
            "Control+Shift+KeyA",
            &mut |_| unreachable!("equivalent chord must not replace registration"),
            &mut |_| Err(HotkeyUpdateError::Persistence("failure".into())),
        );
        assert!(result.is_err());
        assert_eq!(state.manager, Some("old"));
        assert_eq!(state.current_hotkey, "ctrl+shift+a");
    }

    #[test]
    fn test_hotkey_update_failed_registration_returns_error_without_persisting() {
        let (tx, rx) = async_channel::unbounded();
        let response = request_hotkey_update(&tx, "ctrl+shift+b".into()).expect("request");
        let mut state = HotkeyListenerState {
            current_hotkey: "ctrl+shift+a".into(),
            manager: Some("old"),
        };
        let mut persisted = false;
        process_listener_message(
            &mut state,
            ListenerMessage::UpdateHotkey(rx.try_recv().expect("request")),
            &mut |_| None,
            &mut |_| {
                persisted = true;
                Ok(())
            },
            &mut |_| {},
        );
        assert!(matches!(
            response.try_recv(),
            Ok(Err(HotkeyUpdateError::Registration))
        ));
        assert!(!persisted);
        assert_eq!(state.manager, Some("old"));
    }

    #[test]
    fn test_hotkey_update_failed_persistence_preserves_old_registration() {
        let mut state = HotkeyListenerState {
            current_hotkey: "ctrl+shift+a".into(),
            manager: Some("old"),
        };
        let result = update_hotkey(
            &mut state,
            "ctrl+shift+b",
            &mut |_| Some("new"),
            &mut |_| Err(HotkeyUpdateError::Persistence("fixture failure".into())),
        );
        assert!(matches!(result, Err(HotkeyUpdateError::Persistence(_))));
        assert_eq!(state.current_hotkey, "ctrl+shift+a");
        assert_eq!(state.manager, Some("old"));
    }

    #[test]
    fn test_hotkey_update_persistence_failure_drops_candidate_and_retry_keeps_old_live_until_commit()
     {
        use std::rc::Rc;
        struct Manager(Rc<Cell<bool>>);
        impl Drop for Manager {
            fn drop(&mut self) {
                self.0.set(true);
            }
        }
        let old_dropped = Rc::new(Cell::new(false));
        let candidate_dropped = Rc::new(Cell::new(false));
        let mut state = HotkeyListenerState {
            current_hotkey: "ctrl+shift+a".into(),
            manager: Some(Manager(old_dropped.clone())),
        };
        let result = update_hotkey(
            &mut state,
            "ctrl+shift+b",
            &mut |_| {
                assert!(!old_dropped.get());
                Some(Manager(candidate_dropped.clone()))
            },
            &mut |_| {
                assert!(!old_dropped.get());
                Err(HotkeyUpdateError::Persistence("fixture failure".into()))
            },
        );
        assert!(result.is_err());
        assert!(!old_dropped.get());
        assert!(candidate_dropped.get());
        update_hotkey(
            &mut state,
            "ctrl+shift+b",
            &mut |_| {
                assert!(!old_dropped.get());
                Some(Manager(Rc::new(Cell::new(false))))
            },
            &mut |_| {
                assert!(!old_dropped.get());
                Ok(())
            },
        )
        .expect("retry");
        assert!(old_dropped.get());
        assert_eq!(state.current_hotkey, "ctrl+shift+b");
    }

    #[test]
    fn test_hotkey_update_closed_channel_returns_error() {
        let (tx, rx) = async_channel::unbounded();
        drop(rx);
        assert!(matches!(
            request_hotkey_update(&tx, "ctrl+shift+a".into()),
            Err(HotkeyUpdateError::Disconnected)
        ));
    }

    #[test]
    fn test_listener_message_registration_failure_preserves_previous_hotkey() {
        let mut state = HotkeyListenerState {
            current_hotkey: "ctrl+shift+a".to_string(),
            manager: Some("old-manager"),
        };
        process_listener_message(
            &mut state,
            update_message("ctrl+shift+b"),
            &mut |_| None,
            &mut |_| Ok(()),
            &mut |_| {},
        );
        assert_eq!(state.current_hotkey, "ctrl+shift+a");
        assert_eq!(state.manager, Some("old-manager"));
    }

    #[test]
    fn test_listener_message_missing_manager_same_hotkey_retries() {
        let mut state = HotkeyListenerState {
            current_hotkey: "ctrl+shift+a".to_string(),
            manager: None,
        };
        process_listener_message(
            &mut state,
            update_message("ctrl+shift+a"),
            &mut |_| Some("recovered-manager"),
            &mut |_| Ok(()),
            &mut |_| {},
        );
        assert_eq!(state.manager, Some("recovered-manager"));
    }

    #[rstest]
    #[case("")]
    #[case("not+a+valid+hotkey")]
    fn test_register_hotkey_invalid_input_returns_none(#[case] hotkey: &str) {
        assert!(register_hotkey(hotkey).is_none());
    }

    #[test]
    fn test_listener_message_update_hotkey_re_registers_and_updates_state() {
        let mut state = HotkeyListenerState {
            current_hotkey: "ctrl+shift+a".to_string(),
            manager: Some("initial-manager"),
        };
        let mut registered_hotkeys = Vec::new();
        let callback_count = Cell::new(0);

        process_listener_message(
            &mut state,
            update_message("ctrl+shift+b"),
            &mut |hotkey| {
                registered_hotkeys.push(hotkey.to_string());
                Some("updated-manager")
            },
            &mut |_| Ok(()),
            &mut |_| callback_count.set(callback_count.get() + 1),
        );

        assert_eq!(state.current_hotkey, "ctrl+shift+b");
        assert_eq!(state.manager, Some("updated-manager"));
        assert_eq!(registered_hotkeys, vec!["ctrl+shift+b"]);
        assert_eq!(callback_count.get(), 0);
    }

    #[test]
    fn test_listener_message_update_hotkey_same_value_skips_reregistration() {
        let mut state = HotkeyListenerState {
            current_hotkey: "ctrl+shift+a".to_string(),
            manager: Some("initial-manager"),
        };
        let mut register_call_count = 0;

        process_listener_message(
            &mut state,
            update_message("ctrl+shift+a"),
            &mut |_| {
                register_call_count += 1;
                Some("updated-manager")
            },
            &mut |_| Ok(()),
            &mut |_| {},
        );

        assert_eq!(state.current_hotkey, "ctrl+shift+a");
        assert_eq!(state.manager, Some("initial-manager"));
        assert_eq!(register_call_count, 0);
    }

    #[rstest]
    #[case(HotKeyState::Pressed, 1)]
    #[case(HotKeyState::Released, 0)]
    fn test_listener_message_hotkey_event_state_matches_callback_trigger(
        #[case] state: HotKeyState,
        #[case] expected_callback_count: usize,
    ) {
        let mut listener_state = HotkeyListenerState {
            current_hotkey: "ctrl+shift+a".to_string(),
            manager: Some("manager"),
        };
        let callback_count = Cell::new(0);

        process_listener_message(
            &mut listener_state,
            ListenerMessage::HotkeyEvent(GlobalHotKeyEvent {
                id: "ctrl+shift+a".parse::<HotKey>().expect("hotkey").id,
                state,
            }),
            &mut |_| Some("updated-manager"),
            &mut |_| Ok(()),
            &mut |_| callback_count.set(callback_count.get() + 1),
        );

        assert_eq!(callback_count.get(), expected_callback_count);
        assert_eq!(listener_state.manager, Some("manager"));
    }
}

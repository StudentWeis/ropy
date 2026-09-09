use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui::{App, AsyncApp, ReadGlobal as _};
use thiserror::Error;

use crate::config::Settings;

#[derive(Clone)]
enum ListenerMessage {
    UpdateHotkey(HotkeyUpdate),
    HotkeyEvent(GlobalHotKeyEvent),
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
    F: Fn(&AsyncApp) + 'static,
{
    let (tx, rx) = async_channel::unbounded::<HotkeyUpdate>();
    let (message_tx, message_rx) = async_channel::unbounded::<ListenerMessage>();

    spawn_hotkey_event_forwarder(message_tx.clone());
    spawn_hotkey_update_forwarder(rx, message_tx);

    cx.spawn(async move |async_app| {
        let mut state = HotkeyListenerState {
            manager: register_hotkey(&initial_hotkey),
            current_hotkey: initial_hotkey,
        };

        while let Ok(message) = message_rx.recv().await {
            process_listener_message(
                &mut state,
                message,
                &mut register_hotkey,
                &mut |hotkey| {
                    async_app
                        .update(|cx| {
                            let mut settings = Settings::global(cx).clone();
                            settings.hotkey.activation_key = hotkey.to_string();
                            settings.save().map_err(|error| {
                                HotkeyUpdateError::Persistence(error.to_string())
                            })?;
                            cx.set_global(settings);
                            Ok(())
                        })
                        .map_err(|_| HotkeyUpdateError::Disconnected)?
                },
                &mut || on_hotkey(async_app),
            );
        }
    })
    .detach();

    tx
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
    OnHotkey: FnMut(),
{
    match message {
        ListenerMessage::UpdateHotkey(request) => {
            let result = update_hotkey(state, &request.hotkey, register_hotkey, persist_hotkey);
            let _ = request.completion.try_send(result);
        }
        ListenerMessage::HotkeyEvent(event) => {
            if event.state() == HotKeyState::Pressed {
                on_hotkey();
            }
        }
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
            &mut || {},
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
            &mut || {},
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
            &mut || {},
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
            &mut || callback_count.set(callback_count.get() + 1),
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
            &mut || {},
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
            ListenerMessage::HotkeyEvent(GlobalHotKeyEvent { id: 42, state }),
            &mut |_| Some("updated-manager"),
            &mut |_| Ok(()),
            &mut || callback_count.set(callback_count.get() + 1),
        );

        assert_eq!(callback_count.get(), expected_callback_count);
        assert_eq!(listener_state.manager, Some("manager"));
    }
}

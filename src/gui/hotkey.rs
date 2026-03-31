use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui::{App, AsyncApp};

#[derive(Clone)]
enum ListenerMessage {
    UpdateHotkey(String),
    HotkeyEvent(GlobalHotKeyEvent),
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
pub fn start_hotkey_listener<F>(
    initial_hotkey: String,
    cx: &App,
    on_hotkey: F,
) -> async_channel::Sender<String>
where
    F: Fn(&AsyncApp) + 'static,
{
    let (tx, rx) = async_channel::unbounded::<String>();
    let (message_tx, message_rx) = async_channel::unbounded::<ListenerMessage>();

    spawn_hotkey_event_forwarder(message_tx.clone());
    spawn_hotkey_update_forwarder(rx, message_tx);

    cx.spawn(async move |async_app| {
        let mut state = HotkeyListenerState {
            manager: register_hotkey(&initial_hotkey),
            current_hotkey: initial_hotkey,
        };

        while let Ok(message) = message_rx.recv().await {
            process_listener_message(&mut state, message, &mut register_hotkey, &mut || {
                on_hotkey(async_app);
            });
        }
    })
    .detach();

    tx
}

fn process_listener_message<Manager, RegisterHotkey, OnHotkey>(
    state: &mut HotkeyListenerState<Manager>,
    message: ListenerMessage,
    register_hotkey: &mut RegisterHotkey,
    on_hotkey: &mut OnHotkey,
) where
    RegisterHotkey: FnMut(&str) -> Option<Manager>,
    OnHotkey: FnMut(),
{
    match message {
        ListenerMessage::UpdateHotkey(new_hotkey) => {
            if new_hotkey == state.current_hotkey {
                return;
            }

            state.current_hotkey = new_hotkey;
            state.manager = None;
            state.manager = register_hotkey(&state.current_hotkey);
        }
        ListenerMessage::HotkeyEvent(event) => {
            if event.state() == HotKeyState::Pressed {
                on_hotkey();
            }
        }
    }
}

fn spawn_hotkey_event_forwarder(message_tx: async_channel::Sender<ListenerMessage>) {
    let receiver = GlobalHotKeyEvent::receiver().clone();
    let spawn_result = std::thread::Builder::new()
        .name("hotkey-event-forwarder".to_string())
        .spawn(move || {
            while let Ok(event) = receiver.recv() {
                if message_tx
                    .send_blocking(ListenerMessage::HotkeyEvent(event))
                    .is_err()
                {
                    break;
                }
            }
        });

    if let Err(err) = spawn_result {
        tracing::error!(error = %err, "failed to spawn hotkey event forwarder");
    }
}

fn spawn_hotkey_update_forwarder(
    update_rx: async_channel::Receiver<String>,
    message_tx: async_channel::Sender<ListenerMessage>,
) {
    let spawn_result = std::thread::Builder::new()
        .name("hotkey-update-forwarder".to_string())
        .spawn(move || {
            while let Ok(hotkey) = update_rx.recv_blocking() {
                if message_tx
                    .send_blocking(ListenerMessage::UpdateHotkey(hotkey))
                    .is_err()
                {
                    break;
                }
            }
        });

    if let Err(err) = spawn_result {
        tracing::error!(error = %err, "failed to spawn hotkey update forwarder");
    }
}

fn register_hotkey(hotkey_str: &str) -> Option<GlobalHotKeyManager> {
    if hotkey_str.is_empty() {
        return None;
    }
    let manager = match GlobalHotKeyManager::new() {
        Ok(m) => m,
        Err(err) => {
            tracing::error!(error = %err, "failed to create GlobalHotKeyManager");
            return None;
        }
    };
    match hotkey_str.parse::<HotKey>() {
        Ok(hotkey) => {
            if let Err(err) = manager.register(hotkey) {
                tracing::warn!(
                    hotkey = hotkey_str,
                    error = %err,
                    "failed to register hotkey; the hotkey listener will not be available"
                );
                None
            } else {
                Some(manager)
            }
        }
        Err(err) => {
            tracing::warn!(
                hotkey = hotkey_str,
                error = %err,
                "failed to parse hotkey; the hotkey listener will not be available"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use rstest::rstest;

    use super::*;

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
            ListenerMessage::UpdateHotkey("ctrl+shift+b".to_string()),
            &mut |hotkey| {
                registered_hotkeys.push(hotkey.to_string());
                Some("updated-manager")
            },
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
            ListenerMessage::UpdateHotkey("ctrl+shift+a".to_string()),
            &mut |_| {
                register_call_count += 1;
                Some("updated-manager")
            },
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
            &mut || callback_count.set(callback_count.get() + 1),
        );

        assert_eq!(callback_count.get(), expected_callback_count);
        assert_eq!(listener_state.manager, Some("manager"));
    }
}

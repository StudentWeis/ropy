use global_hotkey::HotKeyState;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use gpui::{BackgroundExecutor, ForegroundExecutor};
use std::time::Duration;

/// Start a global hotkey listener in a background task with a custom callback.
///
/// Registers Ctrl+Shift+D and invokes the provided callback when the hotkey is pressed.
/// The manager is kept alive inside the spawned task so it isn't dropped.
pub fn start_hotkey_listener<F>(
    fg_executor: ForegroundExecutor,
    bg_executor: BackgroundExecutor,
    on_hotkey: F,
) where
    F: Fn() + 'static,
{
    fg_executor
        .spawn(async move {
            let manager = GlobalHotKeyManager::new().expect("Failed to create GlobalHotKeyManager");
            let hotkey = HotKey::new(Some(Modifiers::SHIFT | Modifiers::CONTROL), Code::KeyD);
            if let Err(err) = manager.register(hotkey) {
                eprintln!(
                "Failed to register hotkey Ctrl+Shift+D: {err}. The hotkey listener will not be available."
            );
            }

            let receiver = GlobalHotKeyEvent::receiver();

            loop {
                // Poll for hotkey events
                if let Ok(event) = receiver.try_recv()
                    && event.state() == HotKeyState::Pressed {
                        on_hotkey();
                    }

                // Small sleep to avoid busy waiting
                bg_executor.timer(Duration::from_millis(50)).await;
            }
        })
        .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use global_hotkey::GlobalHotKeyEvent;

    #[test]
    fn test_hotkey_registration_and_unregistration() {
        // This test verifies registration/unregistration and receiver availability
        let manager = GlobalHotKeyManager::new().unwrap();
        let hotkey = HotKey::new(Some(Modifiers::SHIFT | Modifiers::CONTROL), Code::KeyD);
        assert!(manager.register(hotkey).is_ok());
        let receiver = GlobalHotKeyEvent::receiver();
        assert!(receiver.try_recv().is_err());
        assert!(manager.unregister(hotkey).is_ok());
    }
}

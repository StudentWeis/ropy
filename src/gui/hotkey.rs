use global_hotkey::HotKeyState;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use std::thread;

/// Start a global hotkey listener in a background thread with a custom callback.
///
/// Registers SHIFT+D and invokes the provided callback when the hotkey is pressed.
/// The manager is kept alive inside the spawned thread so it isn't dropped.
///
/// Returns the JoinHandle so the caller can keep the thread alive or join it when needed.
pub fn start_hotkey_listener<F>(on_hotkey: F) -> thread::JoinHandle<()>
where
    F: Fn() + Send + 'static,
{
    let manager = GlobalHotKeyManager::new().expect("Failed to create GlobalHotKeyManager");
    let hotkey = HotKey::new(Some(Modifiers::SHIFT), Code::KeyD);
    if let Err(err) = manager.register(hotkey) {
        eprintln!(
            "Failed to register hotkey SHIFT+D: {err}. The hotkey listener will not be available."
        );
    }
    thread::spawn(move || {
        let _manager = manager; // Keep the manager alive
        let receiver = GlobalHotKeyEvent::receiver();
        while let Ok(event) = receiver.recv() {
            // Only handle pressed state to avoid duplicate events
            if event.state() == HotKeyState::Pressed {
                on_hotkey();
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use global_hotkey::GlobalHotKeyEvent;

    #[test]
    fn test_hotkey_registration_and_unregistration() {
        // This test verifies registration/unregistration and receiver availability
        let manager = GlobalHotKeyManager::new().unwrap();
        let hotkey = HotKey::new(Some(Modifiers::SHIFT), Code::KeyD);
        assert!(manager.register(hotkey).is_ok());
        let receiver = GlobalHotKeyEvent::receiver();
        assert!(receiver.try_recv().is_err());
        assert!(manager.unregister(hotkey).is_ok());
    }
}

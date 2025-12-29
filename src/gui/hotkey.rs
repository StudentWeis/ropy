use global_hotkey::HotKeyState;
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use gpui::{BackgroundExecutor, ForegroundExecutor};
use std::time::Duration;

/// Start a global hotkey listener in a background task with a custom callback.
///
/// Registers the configured hotkey and invokes the provided callback when the hotkey is pressed.
/// Returns a sender to update the hotkey string dynamically.
pub fn start_hotkey_listener<F>(
    initial_hotkey: String,
    fg_executor: ForegroundExecutor,
    bg_executor: BackgroundExecutor,
    on_hotkey: F,
) -> async_channel::Sender<String>
where
    F: Fn() + 'static,
{
    let (tx, rx) = async_channel::unbounded::<String>();
    fg_executor
        .spawn(async move {
            let mut current_hotkey = initial_hotkey;
            let mut _manage_handle = register_hotkey(&current_hotkey);
            let receiver = GlobalHotKeyEvent::receiver();
            loop {
                // Check for hotkey updates
                let mut updated = false;
                while let Ok(new_hotkey) = rx.try_recv() {
                    current_hotkey = new_hotkey;
                    updated = true;
                }

                if updated {
                    drop(_manage_handle);
                    _manage_handle = register_hotkey(&current_hotkey);
                }

                // Poll for hotkey events
                if let Ok(event) = receiver.try_recv()
                    && event.state() == HotKeyState::Pressed
                {
                    on_hotkey();
                }

                // Small sleep to avoid busy waiting
                bg_executor.timer(Duration::from_millis(50)).await;
            }
        })
        .detach();
    tx
}

fn register_hotkey(hotkey_str: &str) -> Option<GlobalHotKeyManager> {
    if hotkey_str.is_empty() {
        return None;
    }
    let manager = match GlobalHotKeyManager::new() {
        Ok(m) => m,
        Err(err) => {
            eprintln!("Failed to create GlobalHotKeyManager: {err}");
            return None;
        }
    };
    match hotkey_str.parse::<HotKey>() {
        Ok(hotkey) => {
            if let Err(err) = manager.register(hotkey) {
                eprintln!(
                    "Failed to register hotkey {}: {}. The hotkey listener will not be available.",
                    hotkey_str, err
                );
                None
            } else {
                Some(manager)
            }
        }
        Err(err) => {
            eprintln!(
                "Failed to parse hotkey {}: {}. The hotkey listener will not be available.",
                hotkey_str, err
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use global_hotkey::GlobalHotKeyEvent;

    #[test]
    fn test_hotkey_registration_and_unregistration() {
        // This test verifies registration/unregistration and receiver availability
        let manager = GlobalHotKeyManager::new().unwrap();
        let hotkey: HotKey = "control+shift+d".parse().unwrap();
        assert!(manager.register(hotkey).is_ok());
        let receiver = GlobalHotKeyEvent::receiver();
        assert!(receiver.try_recv().is_err());
        assert!(manager.unregister(hotkey).is_ok());
    }
}

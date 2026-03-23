use std::time::Duration;

use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState, hotkey::HotKey};
use gpui::{App, AsyncApp};

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
    cx.spawn(async move |async_app| {
        let bg_executor = async_app.background_executor().clone();
        let mut current_hotkey = initial_hotkey;
        let mut manage_handle = register_hotkey(&current_hotkey);
        let receiver = GlobalHotKeyEvent::receiver();
        loop {
            // Check for hotkey updates
            let mut updated = false;
            while let Ok(new_hotkey) = rx.try_recv() {
                current_hotkey = new_hotkey;
                updated = true;
            }

            if updated {
                drop(manage_handle);
                manage_handle = register_hotkey(&current_hotkey);
            }

            // Poll for hotkey events
            if let Ok(event) = receiver.try_recv()
                && event.state() == HotKeyState::Pressed
            {
                on_hotkey(async_app);
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
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used)]
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

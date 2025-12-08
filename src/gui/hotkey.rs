use global_hotkey::HotKeyState;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager};
use std::thread;

/// Start a global hotkey listener in a background thread with a custom callback.
///
    /// Registers Ctrl+Shift+D and invokes the provided callback when the hotkey is pressed.
/// The manager is kept alive inside the spawned thread so it isn't dropped.
///
/// Returns the JoinHandle so the caller can keep the thread alive or join it when needed.
pub fn start_hotkey_listener<F>(on_hotkey: F) -> thread::JoinHandle<()>
where
    F: Fn() + Send + 'static,
{
    thread::spawn(move || {
        let manager = GlobalHotKeyManager::new().expect("Failed to create GlobalHotKeyManager");
    let hotkey = HotKey::new(Some(Modifiers::SHIFT | Modifiers::CONTROL), Code::KeyD);
        if let Err(err) = manager.register(hotkey) {
            eprintln!(
                "Failed to register hotkey Ctrl+Shift+D: {err}. The hotkey listener will not be available."
            );
        }

        let receiver = GlobalHotKeyEvent::receiver();
        
        // On Windows, we need a message loop for hotkeys to work
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                PeekMessageW, TranslateMessage, DispatchMessageW, MSG, PM_REMOVE,
            };
            use std::time::Duration;
            
            loop {
                // Poll for hotkey events
                if let Ok(event) = receiver.try_recv() {
                    if event.state() == HotKeyState::Pressed {
                        on_hotkey();
                    }
                }
                
                // Process Windows messages without blocking
                unsafe {
                    let mut msg: MSG = std::mem::zeroed();
                    while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
                
                // Small sleep to avoid busy waiting
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        
        // On macOS and other platforms, simple event loop
        #[cfg(not(target_os = "windows"))]
        {
            while let Ok(event) = receiver.recv() {
                if event.state() == HotKeyState::Pressed {
                    on_hotkey();
                }
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
    let hotkey = HotKey::new(Some(Modifiers::SHIFT | Modifiers::CONTROL), Code::KeyD);
        assert!(manager.register(hotkey).is_ok());
        let receiver = GlobalHotKeyEvent::receiver();
        assert!(receiver.try_recv().is_err());
        assert!(manager.unregister(hotkey).is_ok());
    }
}

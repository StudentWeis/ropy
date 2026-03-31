use gpui::{Context, Window};

/// Spawn a named thread that runs a blocking receive loop and forwards mapped
/// events to an `async_channel::Sender`.
///
/// `receive_loop` is called with a closure that the caller invokes for each
/// received event. The closure accepts `Option<T>`: `None` values are skipped
/// (filtered), and `Some(value)` values are forwarded. It returns `true` while
/// the sender is still connected; the caller should stop when it returns `false`.
///
/// # Example
///
/// ```ignore
/// let receiver = GlobalHotKeyEvent::receiver().clone();
/// spawn_event_forwarder("hotkey-forwarder", sender, move |forward| {
///     while let Ok(event) = receiver.recv() {
///         if !forward(Some(ListenerMessage::HotkeyEvent(event))) {
///             break;
///         }
///     }
/// });
/// ```
pub fn spawn_event_forwarder<T, F>(
    thread_name: &str,
    sender: async_channel::Sender<T>,
    receive_loop: F,
) where
    T: Send + 'static,
    F: FnOnce(&dyn Fn(Option<T>) -> bool) + Send + 'static,
{
    let spawn_result = std::thread::Builder::new()
        .name(thread_name.to_string())
        .spawn(move || {
            receive_loop(&|mapped| {
                let Some(value) = mapped else {
                    return true;
                };
                sender.send_blocking(value).is_ok()
            });
        });

    if let Err(err) = spawn_result {
        tracing::error!(
            thread_name = thread_name,
            error = %err,
            "failed to spawn event forwarder"
        );
    }
}
#[cfg(target_os = "windows")]
use {
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    windows_sys::Win32::UI::WindowsAndMessaging::{
        SW_HIDE, SW_RESTORE, SetForegroundWindow, ShowWindow,
    },
};

/// Hide the window based on the platform
#[allow(unused_variables, clippy::needless_pass_by_ref_mut)]
pub fn hide_window<T>(window: &mut Window, cx: &Context<T>, pinned: bool) {
    if pinned {
        return;
    }
    #[cfg(target_os = "windows")]
    if let Ok(window_handle) = HasWindowHandle::window_handle(window)
        && let RawWindowHandle::Win32(win32_handle) = window_handle.as_raw()
    {
        let hwnd = win32_handle.hwnd.get() as *mut std::ffi::c_void;
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
    }

    #[cfg(target_os = "macos")]
    cx.hide();

    #[cfg(target_os = "linux")]
    if let Some(x11) = crate::app::X11_INSTANCE.get() {
        if let Err(e) = x11.hide_window() {
            tracing::warn!(error = %e, "failed to hide window");
        }
    }
}

/// Activate the window based on the platform
#[allow(unused_variables, clippy::needless_pass_by_ref_mut)]
pub fn active_window<T>(window: &mut Window, cx: &Context<T>) {
    #[cfg(target_os = "windows")]
    if let Ok(window_handle) = HasWindowHandle::window_handle(window)
        && let RawWindowHandle::Win32(win32_handle) = window_handle.as_raw()
    {
        let hwnd = win32_handle.hwnd.get() as *mut std::ffi::c_void;
        unsafe {
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }

    #[cfg(target_os = "macos")]
    cx.activate(true);

    #[cfg(target_os = "linux")]
    if let Some(x11) = crate::app::X11_INSTANCE.get() {
        if let Err(e) = x11.display_and_activate_window() {
            tracing::warn!(error = %e, "failed to activate window");
        }
    }
}

/// Set the window to be always on top
#[allow(unused_variables)]
#[cfg(not(target_os = "macos"))]
pub fn set_always_on_top(window: &Window, pinned: bool) {
    #[cfg(target_os = "windows")]
    if let Ok(window_handle) = HasWindowHandle::window_handle(window)
        && let RawWindowHandle::Win32(win32_handle) = window_handle.as_raw()
    {
        let hwnd = win32_handle.hwnd.get() as *mut std::ffi::c_void;
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
            };
            let hwnd_insert_after: *mut std::ffi::c_void =
                if pinned { HWND_TOPMOST } else { HWND_NOTOPMOST };
            SetWindowPos(hwnd, hwnd_insert_after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(x11) = crate::app::X11_INSTANCE.get() {
            if let Err(e) = x11.set_always_on_top(pinned) {
                tracing::warn!(error = %e, "failed to set always on top");
            }
        }
    }
}

/// Start dragging the window
#[cfg(target_os = "windows")]
pub fn start_window_drag(window: &Window) {
    use windows_sys::Win32::UI::{
        Input::KeyboardAndMouse::ReleaseCapture,
        WindowsAndMessaging::{HTCAPTION, PostMessageA, WM_NCLBUTTONDOWN},
    };
    if let Ok(window_handle) = HasWindowHandle::window_handle(window)
        && let RawWindowHandle::Win32(win32_handle) = window_handle.as_raw()
    {
        let hwnd = win32_handle.hwnd.get() as *mut std::ffi::c_void;
        unsafe {
            ReleaseCapture();
            PostMessageA(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
        }
    }
}

#[cfg(target_os = "macos")]
pub fn set_activation_policy_accessory() {
    use objc2::{class, msg_send, runtime::AnyObject};
    unsafe {
        // Config the app to be accessory (no dock icon & cmd tab)
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _succeeded: bool = msg_send![app, setActivationPolicy: 1isize];
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, time::Duration};

    use super::spawn_event_forwarder;

    #[test]
    fn test_spawn_event_forwarder_none_input_skips_value() {
        let (sender, receiver) = async_channel::unbounded();
        let (done_tx, done_rx) = mpsc::channel();

        spawn_event_forwarder("test-forwarder-filter", sender, move |forward| {
            let first_result = forward(None);
            let second_result = forward(Some(42));
            assert!(done_tx.send((first_result, second_result)).is_ok());
        });

        let (first_result, second_result) = match done_rx.recv_timeout(Duration::from_secs(1)) {
            Ok(results) => results,
            Err(err) => panic!("forwarder thread should finish promptly: {err}"),
        };

        assert!(first_result);
        assert!(second_result);
        assert_eq!(receiver.recv_blocking(), Ok(42));
        assert!(receiver.recv_blocking().is_err());
    }

    #[test]
    fn test_spawn_event_forwarder_receiver_disconnected_returns_false() {
        let (sender, receiver) = async_channel::unbounded::<usize>();
        let (done_tx, done_rx) = mpsc::channel();
        drop(receiver);

        spawn_event_forwarder("test-forwarder-disconnect", sender, move |forward| {
            let send_result = forward(Some(42));
            assert!(done_tx.send(send_result).is_ok());
        });

        let send_result = match done_rx.recv_timeout(Duration::from_secs(1)) {
            Ok(result) => result,
            Err(err) => panic!("forwarder thread should report disconnected result: {err}"),
        };

        assert!(!send_result);
    }
}

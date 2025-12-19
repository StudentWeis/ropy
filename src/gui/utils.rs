use gpui::{Context, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_HIDE, SW_RESTORE, SetForegroundWindow, PostMessageA, WM_NCLBUTTONDOWN, HTCAPTION};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;

/// Hide the window based on the platform
pub fn hide_window<T>(_window: &mut Window, _cx: &mut Context<T>) {
    #[cfg(target_os = "windows")]
    if let Ok(handle) = _window.window_handle() {
        if let RawWindowHandle::Win32(handle) = handle.as_raw() {
            let hwnd = handle.hwnd.get() as *mut std::ffi::c_void;
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
    #[cfg(target_os = "macos")]
    _cx.hide();
}

/// Activate the window based on the platform
pub fn active_window<T>(_window: &mut Window, _cx: &mut Context<T>) {
    #[cfg(target_os = "windows")]
    if let Ok(handle) = _window.window_handle() {
        if let RawWindowHandle::Win32(handle) = handle.as_raw() {
            let hwnd = handle.hwnd.get() as *mut std::ffi::c_void;
            unsafe {
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            }
        }
    }
    #[cfg(target_os = "macos")]
    _cx.activate(true);
}

/// Start dragging the window (Windows only)
pub fn start_window_drag(window: &mut Window, _cx: &mut gpui::App) {
    #[cfg(target_os = "windows")]
    if let Ok(handle) = window.window_handle() {
        if let RawWindowHandle::Win32(handle) = handle.as_raw() {
            let hwnd = handle.hwnd.get() as *mut std::ffi::c_void;
            unsafe {
                ReleaseCapture();
                PostMessageA(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
            }
        }
    }
}

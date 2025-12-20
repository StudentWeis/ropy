use gpui::{Context, Window};

#[cfg(target_os = "windows")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SW_HIDE, SW_RESTORE, SetForegroundWindow, ShowWindow,
};

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

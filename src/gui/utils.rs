use gpui::{Context, Window};

#[cfg(not(target_os = "linux"))]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    HTCAPTION, PostMessageA, SW_HIDE, SW_RESTORE, SetForegroundWindow, ShowWindow, WM_NCLBUTTONDOWN,
};

#[cfg(target_os = "macos")]
use objc2::{msg_send, runtime::AnyObject};

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

/// Set the window to be always on top
pub fn set_always_on_top<T>(_window: &mut Window, _cx: &mut Context<T>, always_on_top: bool) {
    #[cfg(target_os = "windows")]
    if let Ok(handle) = _window.window_handle() {
        if let RawWindowHandle::Win32(handle) = handle.as_raw() {
            let hwnd = handle.hwnd.get() as *mut std::ffi::c_void;
            unsafe {
                use windows_sys::Win32::UI::WindowsAndMessaging::{
                    HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
                };
                let hwnd_insert_after = if always_on_top {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                };
                SetWindowPos(hwnd, hwnd_insert_after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
            }
        }
    }
    #[cfg(target_os = "macos")]
    if let Ok(handle) = _window.window_handle()
        && let RawWindowHandle::AppKit(handle) = handle.as_raw()
    {
        // NSFloatingWindowLevel = 3, NSNormalWindowLevel = 0
        let level: isize = if always_on_top { 3 } else { 0 };
        let ns_view = handle.ns_view.as_ptr() as *mut AnyObject;
        unsafe {
            let ns_window: *mut AnyObject = msg_send![ns_view, window];
            if !ns_window.is_null() {
                let _: () = msg_send![ns_window, setLevel: level];
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Some(x11) = crate::gui::app::X11.get() {
            if let Err(e) = x11.set_always_on_top(always_on_top) {
                eprintln!("[ropy] Failed to set always on top: {e}")
            }
        }
    }
}

/// Start dragging the window
#[cfg(target_os = "windows")]
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

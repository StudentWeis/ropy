use gpui::{Context, Window};
#[cfg(target_os = "windows")]
use {
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    windows_sys::Win32::UI::WindowsAndMessaging::{
        SW_HIDE, SW_RESTORE, SetForegroundWindow, ShowWindow,
    },
};

/// Hide the window based on the platform
pub fn hide_window<T>(_window: &mut Window, _cx: &mut Context<T>, pinned: bool) {
    if pinned {
        return;
    }
    #[cfg(target_os = "windows")]
    if let Ok(handle) = _window.window_handle()
        && let RawWindowHandle::Win32(handle) = handle.as_raw()
    {
        let hwnd = handle.hwnd.get() as *mut std::ffi::c_void;
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
    }

    #[cfg(target_os = "macos")]
    _cx.hide();

    #[cfg(target_os = "linux")]
    if let Some(x11) = crate::gui::app::X11.get() {
        if let Err(e) = x11.hide_window() {
            eprintln!("[ropy] Failed to hide window: {e}")
        }
    }
}

/// Activate the window based on the platform
pub fn active_window<T>(_window: &mut Window, _cx: &mut Context<T>) {
    #[cfg(target_os = "windows")]
    if let Ok(handle) = _window.window_handle()
        && let RawWindowHandle::Win32(handle) = handle.as_raw()
    {
        let hwnd = handle.hwnd.get() as *mut std::ffi::c_void;
        unsafe {
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }

    #[cfg(target_os = "macos")]
    _cx.activate(true);

    #[cfg(target_os = "linux")]
    if let Some(x11) = crate::gui::app::X11.get() {
        if let Err(e) = x11.display_and_activate_window() {
            eprintln!("[ropy] Failed to activate window: {e}")
        }
    }
}

/// Set the window to be always on top
pub fn set_always_on_top<T>(_window: &mut Window, _cx: &mut Context<T>, _pinned: bool) {
    #[cfg(target_os = "windows")]
    if let Ok(handle) = _window.window_handle()
        && let RawWindowHandle::Win32(handle) = handle.as_raw()
    {
        let hwnd = handle.hwnd.get() as *mut std::ffi::c_void;
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
            };
            let hwnd_insert_after = if _pinned {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            };
            SetWindowPos(hwnd, hwnd_insert_after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
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
    use windows_sys::Win32::UI::{
        Input::KeyboardAndMouse::ReleaseCapture,
        WindowsAndMessaging::{HTCAPTION, PostMessageA, WM_NCLBUTTONDOWN},
    };
    if let Ok(handle) = window.window_handle()
        && let RawWindowHandle::Win32(handle) = handle.as_raw()
    {
        let hwnd = handle.hwnd.get() as *mut std::ffi::c_void;
        unsafe {
            ReleaseCapture();
            PostMessageA(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
        }
    }
}

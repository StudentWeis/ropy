use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

use windows_sys::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
    System::Threading::CreateMutexW,
    UI::WindowsAndMessaging::{FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow},
};

use crate::gui::app::MAIN_WINDOW_TITLE;

pub fn ensure_single_instance() -> bool {
    let mutex_name = "RopySingleInstanceMutex";
    let wide_name: Vec<u16> = OsStr::new(mutex_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: every wide string passed in is null-terminated; window
    // handles returned by `FindWindowW` are only used while this stack
    // frame is live, so neither `ShowWindow` nor `SetForegroundWindow`
    // can outlive them.
    unsafe {
        let mutex = CreateMutexW(std::ptr::null(), 0, wide_name.as_ptr());
        if mutex.is_null() {
            return false;
        }

        if GetLastError() == ERROR_ALREADY_EXISTS {
            // GPUI's class name is shared by every GPUI application. Match
            // Ropy's native window title as well so another GPUI process is
            // never activated by mistake.
            let class_name = OsStr::new("Zed::Window")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let window_title = OsStr::new(MAIN_WINDOW_TITLE)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let hwnd = FindWindowW(class_name.as_ptr(), window_title.as_ptr());
            if !hwnd.is_null() {
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            }
            return false;
        }
        true
    }
}

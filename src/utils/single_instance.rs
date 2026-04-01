use std::{ffi::OsStr, os::windows::ffi::OsStrExt};

use windows_sys::Win32::{
    Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
    System::Threading::CreateMutexW,
    UI::WindowsAndMessaging::{FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow},
};

pub fn ensure_single_instance() -> bool {
    let mutex_name = "RopySingleInstanceMutex";
    let wide_name: Vec<u16> = OsStr::new(mutex_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: CreateMutexW is called with a valid null pointer for security attributes,
    // zero for initial owner (not owned), and a null-terminated wide string for the mutex name.
    // The mutex name is valid UTF-16 with null terminator. GetLastError is safe to call anytime.
    // FindWindowW is called with a valid null-terminated class name and null window name,
    // which searches for any window of the specified class. ShowWindow and SetForegroundWindow
    // are safe to call with any window handle; they may fail silently if the handle is invalid
    // or the operation is not permitted, but will not cause undefined behavior.
    unsafe {
        let mutex = CreateMutexW(std::ptr::null(), 0, wide_name.as_ptr());
        if mutex.is_null() {
            return false;
        }

        if GetLastError() == ERROR_ALREADY_EXISTS {
            // Try to activate existing window
            let class_name = OsStr::new("Zed::Window")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let hwnd = FindWindowW(class_name.as_ptr(), std::ptr::null());
            if !hwnd.is_null() {
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            }
            return false;
        }
        true
    }
}

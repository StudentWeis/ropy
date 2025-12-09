use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, MB_OK, MessageBoxW, SW_RESTORE, SetForegroundWindow, ShowWindow,
};

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

pub fn ensure_single_instance() -> bool {
    let mutex_name = "RopySingleInstanceMutex";
    let wide_name: Vec<u16> = OsStr::new(mutex_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let mutex = CreateMutexW(std::ptr::null(), 0, wide_name.as_ptr());
        if mutex.is_null() {
            return false;
        }

        if GetLastError() == ERROR_ALREADY_EXISTS {
            // 尝试激活现有窗口
            let class_name = OsStr::new("Zed::Window")
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<_>>();
            let hwnd = FindWindowW(class_name.as_ptr(), std::ptr::null());
            if !hwnd.is_null() {
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            } else {
                // 回退到消息框
                let title = OsStr::new("Ropy")
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect::<Vec<_>>();
                let message = OsStr::new("Ropy 已经在运行中。请使用热键来显示窗口。")
                    .encode_wide()
                    .chain(std::iter::once(0))
                    .collect::<Vec<_>>();
                MessageBoxW(
                    std::ptr::null_mut(),
                    message.as_ptr(),
                    title.as_ptr(),
                    MB_OK,
                );
            }
            return false;
        }

        // 保持互斥锁，直到程序退出
        let _ = mutex;
        true
    }
}

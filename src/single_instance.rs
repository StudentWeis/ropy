use windows_sys::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows_sys::Win32::System::Threading::CreateMutexW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    FindWindowW, SW_RESTORE, SetForegroundWindow, ShowWindow, //MessageBoxW, MB_OK,
};

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;


#[cfg(target_os = "windows")]
fn simulate_hotkey() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_SHIFT,
    };

    // Virtual key code for 'D' key
    const VK_D: u16 = 0x44;

    unsafe {
        let mut inputs: [INPUT; 6] = std::mem::zeroed();

        // Press Ctrl
        inputs[0].r#type = INPUT_KEYBOARD;
        inputs[0].Anonymous.ki = KEYBDINPUT {
            wVk: VK_CONTROL,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: 0,
        };

        // Press Shift
        inputs[1].r#type = INPUT_KEYBOARD;
        inputs[1].Anonymous.ki = KEYBDINPUT {
            wVk: VK_SHIFT,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: 0,
        };

        // Press D
        inputs[2].r#type = INPUT_KEYBOARD;
        inputs[2].Anonymous.ki = KEYBDINPUT {
            wVk: VK_D,
            wScan: 0,
            dwFlags: 0,
            time: 0,
            dwExtraInfo: 0,
        };

        // Release D
        inputs[3].r#type = INPUT_KEYBOARD;
        inputs[3].Anonymous.ki = KEYBDINPUT {
            wVk: VK_D,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };

        // Release Shift
        inputs[4].r#type = INPUT_KEYBOARD;
        inputs[4].Anonymous.ki = KEYBDINPUT {
            wVk: VK_SHIFT,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };

        // Release Ctrl
        inputs[5].r#type = INPUT_KEYBOARD;
        inputs[5].Anonymous.ki = KEYBDINPUT {
            wVk: VK_CONTROL,
            wScan: 0,
            dwFlags: KEYEVENTF_KEYUP,
            time: 0,
            dwExtraInfo: 0,
        };

        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }

    // Wait a bit for the hotkey to be processed
    std::thread::sleep(std::time::Duration::from_millis(100));
}


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
                simulate_hotkey();
                // 回退到消息框
                // let title = OsStr::new("Ropy")
                //     .encode_wide()
                //     .chain(std::iter::once(0))
                //     .collect::<Vec<_>>();
                // let message = OsStr::new("Ropy 已经在运行中。请使用热键来显示窗口。")
                //     .encode_wide()
                //     .chain(std::iter::once(0))
                //     .collect::<Vec<_>>();
                // MessageBoxW(
                //     std::ptr::null_mut(),
                //     message.as_ptr(),
                //     title.as_ptr(),
                //     MB_OK,
                // );
            }
            return false;
        }

        // 保持互斥锁，直到程序退出
        let _ = mutex;
        true
    }
}

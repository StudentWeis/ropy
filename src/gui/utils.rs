use std::cfg_select;

use gpui::{Context, Hsla, Pixels, Size, Window, hsla};

pub fn surface_with_opacity(color: Hsla, opacity_percent: u8) -> Hsla {
    hsla(
        color.h,
        color.s,
        color.l,
        color.a * (f32::from(opacity_percent) / 100.0),
    )
}

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
    windows_sys::Win32::{
        Foundation::{POINT, RECT},
        Graphics::Gdi::{
            ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO,
            MonitorFromWindow,
        },
        UI::{
            HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI},
            WindowsAndMessaging::{
                GetClientRect, GetWindowRect, IsIconic, SW_HIDE, SW_RESTORE,
                SWP_NOACTIVATE, SWP_NOZORDER, SetForegroundWindow, SetWindowPos, ShowWindow,
                USER_DEFAULT_SCREEN_DPI,
            },
        },
    },
};

#[cfg(any(test, target_os = "windows"))]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct WindowFrameExtents {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(any(test, target_os = "windows"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MonitorWorkArea {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[cfg(any(test, target_os = "windows"))]
impl MonitorWorkArea {
    const fn width(self) -> i32 {
        self.right - self.left
    }

    const fn height(self) -> i32 {
        self.bottom - self.top
    }
}

#[cfg(any(test, target_os = "windows"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ActivationWindowGeometry {
    left: i32,
    top: i32,
    width: i32,
    height: i32,
}

#[cfg(any(test, target_os = "windows"))]
fn calculate_activation_window_geometry(
    work_area: MonitorWorkArea,
    logical_size: Size<Pixels>,
    scale_factor: f32,
    frame_extents: WindowFrameExtents,
) -> ActivationWindowGeometry {
    let scale_factor = if scale_factor.is_finite() && scale_factor.is_sign_positive() {
        scale_factor
    } else {
        1.0
    };
    let client_width =
        ((Into::<f32>::into(logical_size.width) * scale_factor).round() as i32).max(1);
    let client_height =
        ((Into::<f32>::into(logical_size.height) * scale_factor).round() as i32).max(1);
    let width = client_width + frame_extents.left + frame_extents.right;
    let height = client_height + frame_extents.top + frame_extents.bottom;
    let left = work_area.left + ((work_area.width() - width).max(0) / 2);
    let top = work_area.top + ((work_area.height() - height).max(0) / 2);

    ActivationWindowGeometry {
        left,
        top,
        width,
        height,
    }
}

#[cfg(target_os = "windows")]
fn window_frame_extents(hwnd: *mut std::ffi::c_void) -> WindowFrameExtents {
    // SAFETY: `hwnd` comes from GPUI's live window handle; querying geometry APIs is read-only.
    unsafe {
        if IsIconic(hwnd) != 0 {
            return WindowFrameExtents::default();
        }

        let mut window_rect = std::mem::zeroed::<RECT>();
        if GetWindowRect(hwnd, &mut window_rect) == 0 {
            return WindowFrameExtents::default();
        }

        let mut client_rect = std::mem::zeroed::<RECT>();
        if GetClientRect(hwnd, &mut client_rect) == 0 {
            return WindowFrameExtents::default();
        }

        let mut client_top_left = POINT {
            x: client_rect.left,
            y: client_rect.top,
        };
        let mut client_bottom_right = POINT {
            x: client_rect.right,
            y: client_rect.bottom,
        };
        if ClientToScreen(hwnd, &mut client_top_left) == 0
            || ClientToScreen(hwnd, &mut client_bottom_right) == 0
        {
            return WindowFrameExtents::default();
        }

        WindowFrameExtents {
            left: client_top_left.x - window_rect.left,
            top: client_top_left.y - window_rect.top,
            right: window_rect.right - client_bottom_right.x,
            bottom: window_rect.bottom - client_bottom_right.y,
        }
    }
}

#[cfg(target_os = "windows")]
fn current_monitor_scale_factor(hwnd: *mut std::ffi::c_void) -> Option<f32> {
    // SAFETY: `hwnd` is a valid live window handle; `MonitorFromWindow` returns the nearest
    // monitor handle when one is available.
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_null() {
        return None;
    }

    let mut dpi_x = 0;
    let mut dpi_y = 0;
    // SAFETY: `monitor` was returned by `MonitorFromWindow`; the out-pointers are valid.
    let status = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    if status == 0 && dpi_x > 0 && dpi_x == dpi_y {
        return Some(dpi_x as f32 / USER_DEFAULT_SCREEN_DPI as f32);
    }

    // SAFETY: `hwnd` is valid; this is a read-only fallback if the monitor DPI query fails.
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    let effective_dpi = if dpi > 0 {
        dpi
    } else {
        USER_DEFAULT_SCREEN_DPI
    };
    Some(effective_dpi as f32 / USER_DEFAULT_SCREEN_DPI as f32)
}

#[cfg(target_os = "windows")]
fn reset_window_geometry_with_current_monitor_dpi(
    hwnd: *mut std::ffi::c_void,
    logical_size: Size<Pixels>,
) -> bool {
    // SAFETY: `hwnd` is a valid window handle; `MonitorFromWindow` returns the nearest monitor.
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_null() {
        return false;
    }

    let mut monitor_info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..unsafe { std::mem::zeroed() }
    };
    // SAFETY: `monitor` is valid and `monitor_info` points to writable storage.
    if unsafe { GetMonitorInfoW(monitor, &mut monitor_info) } == 0 {
        return false;
    }

    let Some(scale_factor) = current_monitor_scale_factor(hwnd) else {
        return false;
    };
    let geometry = calculate_activation_window_geometry(
        MonitorWorkArea {
            left: monitor_info.rcWork.left,
            top: monitor_info.rcWork.top,
            right: monitor_info.rcWork.right,
            bottom: monitor_info.rcWork.bottom,
        },
        logical_size,
        scale_factor,
        window_frame_extents(hwnd),
    );

    // SAFETY: `hwnd` is valid; we only adjust size and position before the later restore/activate.
    unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            geometry.left,
            geometry.top,
            geometry.width,
            geometry.height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        ) != 0
    }
}

#[allow(unused_variables, clippy::needless_pass_by_ref_mut)]
pub fn reset_window_geometry_for_activation(window: &mut Window, logical_size: Size<Pixels>) {
    #[cfg(target_os = "windows")]
    if let Ok(window_handle) = HasWindowHandle::window_handle(window)
        && let RawWindowHandle::Win32(win32_handle) = window_handle.as_raw()
        && reset_window_geometry_with_current_monitor_dpi(
            win32_handle.hwnd.get() as *mut std::ffi::c_void,
            logical_size,
        )
    {
        return;
    }

    window.resize(logical_size);
}

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
        // SAFETY: The hwnd is obtained from the valid window handle via HasWindowHandle trait.
        // ShowWindow is safe to call with any valid window handle. SW_HIDE simply hides the
        // window without destroying it, which is the intended behavior for hiding the window.
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
    }

    cfg_select! {
        target_os = "macos" => {
            cx.hide();
        }
        target_os = "linux" => {
            if let Some(x11) = crate::app::X11_INSTANCE.get()
                && let Err(e) = x11.hide_window()
            {
                tracing::warn!(error = %e, "failed to hide window");
            }
        }
        _ => {}
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
        // SAFETY: The hwnd comes from gpui's live window handle. Calling ShowWindow and
        // SetForegroundWindow with this handle only changes visibility/focus state and does
        // not transfer ownership or outlive the window's lifetime in this scope.
        unsafe {
            ShowWindow(hwnd, SW_RESTORE);
            SetForegroundWindow(hwnd);
        }
    }

    cfg_select! {
        target_os = "macos" => {
            cx.activate(true);
        }
        target_os = "linux" => {
            if let Some(x11) = crate::app::X11_INSTANCE.get()
                && let Err(e) = x11.display_and_activate_window()
            {
                tracing::warn!(error = %e, "failed to activate window");
            }
        }
        _ => {}
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
        // SAFETY: The hwnd is obtained from the valid window handle via HasWindowHandle trait.
        // SetWindowPos is called with SWP_NOMOVE | SWP_NOSIZE to only change the Z-order
        // (topmost state) without affecting position or size. The hwnd_insert_after value
        // is either HWND_TOPMOST or HWND_NOTOPMOST, both of which are valid system constants.
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
    if let Some(x11) = crate::app::X11_INSTANCE.get()
        && let Err(e) = x11.set_always_on_top(pinned)
    {
        tracing::warn!(error = %e, "failed to set always on top");
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
        // SAFETY: The hwnd is obtained from the valid window handle via HasWindowHandle trait.
        // ReleaseCapture releases any mouse capture and is safe to call even if no capture exists.
        // PostMessageA posts a WM_NCLBUTTONDOWN message with HTCAPTION to simulate dragging the
        // window's title bar. This is a standard technique for implementing custom window drag
        // and is safe as the message is handled by the window manager.
        unsafe {
            ReleaseCapture();
            PostMessageA(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
        }
    }
}

/// Config GPUI to run without a dock icon on macOS
#[cfg(target_os = "macos")]
pub fn set_activation_policy_accessory() {
    use objc2::{class, msg_send, runtime::AnyObject};
    // SAFETY: NSApplication.sharedApplication returns a valid singleton instance that exists
    // for the lifetime of the application. setActivationPolicy: with argument 1 (NSApplicationActivationPolicyAccessory)
    // is a standard API call to configure the app as an accessory (no dock icon, no cmd+tab entry).
    // The msg_send! macro ensures proper ABI compatibility with Objective-C runtime.
    unsafe {
        // Config the app to be accessory (no dock icon & cmd tab)
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _succeeded: bool = msg_send![app, setActivationPolicy: 1isize];
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, time::Duration};

    use gpui::{px, size};

    use super::{
        MonitorWorkArea, WindowFrameExtents, calculate_activation_window_geometry,
        spawn_event_forwarder,
    };

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

    #[test]
    fn test_calculate_activation_window_geometry_centers_window_for_scaled_monitor() {
        let geometry = calculate_activation_window_geometry(
            MonitorWorkArea {
                left: 100,
                top: 50,
                right: 1700,
                bottom: 950,
            },
            size(px(400.0), px(550.0)),
            1.5,
            WindowFrameExtents::default(),
        );

        assert_eq!(geometry.width, 600);
        assert_eq!(geometry.height, 825);
        assert_eq!(geometry.left, 600);
        assert_eq!(geometry.top, 87);
    }

    #[test]
    fn test_calculate_activation_window_geometry_includes_frame_extents() {
        let geometry = calculate_activation_window_geometry(
            MonitorWorkArea {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
            size(px(400.0), px(550.0)),
            1.25,
            WindowFrameExtents {
                left: 6,
                top: 28,
                right: 6,
                bottom: 6,
            },
        );

        assert_eq!(geometry.width, 512);
        assert_eq!(geometry.height, 722);
        assert_eq!(geometry.left, 704);
        assert_eq!(geometry.top, 179);
    }
}

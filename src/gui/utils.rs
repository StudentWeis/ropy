//! Window / display helpers.
//!
//! Most `unsafe` blocks below share the same invariants:
//!   * `hwnd` originates from a live GPUI `Window` handle and never
//!     outlives the enclosing stack frame, so passing it to Win32 APIs
//!     can't dangle.
//!   * Out-pointers (`RECT`, `MONITORINFO`, …) point to locals declared
//!     immediately above, owned for the duration of the call.
//!   * No call transfers ownership of `hwnd` or otherwise mutates global
//!     state that we don't already drive.
//!
//! Per-block `// SAFETY:` notes only call out additional, block-specific
//! invariants on top of the above.

use gpui_kit::{Hsla, Pixels, Size, Window, hsla};

#[cfg(not(target_os = "macos"))]
pub use super::window::set_always_on_top;

pub(crate) fn surface_with_opacity(color: Hsla, opacity_percent: u8) -> Hsla {
    hsla(
        color.h,
        color.s,
        color.l,
        color.a * (f32::from(opacity_percent) / 100.0),
    )
}

/// Bridge a blocking OS receiver onto an `async_channel::Sender` from a
/// dedicated thread. The forwarding closure returns `false` once the
/// async receiver is gone, signalling the loop to break — without that
/// signal the OS callback would keep buffering events for a consumer
/// that no longer exists.
pub(crate) fn spawn_event_forwarder<T, F>(
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
                GetClientRect, GetWindowRect, IsIconic, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos,
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
    #[expect(
        clippy::cast_possible_truncation,
        reason = "physical pixel sizes fit in i32 on every supported platform"
    )]
    let client_width =
        ((Into::<f32>::into(logical_size.width) * scale_factor).round() as i32).max(1);
    #[expect(
        clippy::cast_possible_truncation,
        reason = "physical pixel sizes fit in i32 on every supported platform"
    )]
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
    // SAFETY: see module-level note. Read-only geometry queries.
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
    // SAFETY: see module-level note.
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_null() {
        return None;
    }

    let mut dpi_x = 0;
    let mut dpi_y = 0;
    // SAFETY: `monitor` came from `MonitorFromWindow` above and out-pointers
    // address the locals just declared.
    let status = unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    if status == 0 && dpi_x > 0 && dpi_x == dpi_y {
        return Some(dpi_x as f32 / USER_DEFAULT_SCREEN_DPI as f32);
    }

    // SAFETY: per-monitor DPI failed, fall back to the window-scoped query.
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
    // SAFETY: see module-level note.
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_null() {
        return false;
    }

    let mut monitor_info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..unsafe { std::mem::zeroed() }
    };
    // SAFETY: `monitor` is the handle just returned by `MonitorFromWindow`.
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

    // SAFETY: pure size + position update, no Z-order or focus change
    // (`SWP_NOACTIVATE | SWP_NOZORDER`) so it can't race with the later
    // restore / activate path.
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

pub(crate) fn reset_window_geometry_for_activation(
    window: &mut Window,
    logical_size: Size<Pixels>,
) {
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

/// Let the X11 window manager or Wayland compositor own the move gesture.
#[cfg(target_os = "linux")]
pub fn start_window_drag(window: &Window) {
    window.start_window_move();
}

/// Start an OS-managed drag — the only way to move borderless windows on
/// Windows without re-implementing the entire DWM hit-test loop.
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
        // SAFETY: see module-level note. `WM_NCLBUTTONDOWN` + `HTCAPTION`
        // is the documented "synthesize a title-bar drag" pattern.
        unsafe {
            ReleaseCapture();
            PostMessageA(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
        }
    }
}

/// Configure the menu-bar application before `run`, avoiding a Dock tile at launch.
/// GPUI must own the policy so its launch lifecycle preserves the same value.
#[cfg(target_os = "macos")]
pub(crate) fn set_activation_policy_accessory(platform: &dyn gpui_kit::Platform) {
    platform.set_activation_policy(gpui_kit::ActivationPolicy::Accessory);
}

#[cfg(test)]
#[expect(clippy::panic)]
mod tests {
    use std::{sync::mpsc, time::Duration};

    use gpui_kit::{px, size};

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

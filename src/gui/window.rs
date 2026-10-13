//! Native control of the resident popup: initialization, visibility and focus.
//!
//! The application has one resident popup. Its X11 connection lives for the
//! process lifetime, matching that window, and never escapes this module.
//! Win32 handles below are borrowed from a live GPUI window for each call.

use std::cfg_select;

use gpui_kit::{AsyncApp, Context, Window, WindowHandle, component::Root};
#[cfg(any(target_os = "linux", target_os = "windows"))]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(target_os = "windows")]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SW_HIDE, SW_RESTORE, SetForegroundWindow, SetWindowPos, ShowWindow,
};
#[cfg(target_os = "linux")]
use {super::x11::X11, std::sync::OnceLock};

#[cfg(target_os = "linux")]
static X11_INSTANCE: OnceLock<X11> = OnceLock::new();

/// Install native control before accepting activation requests.
///
/// # Errors
/// Returns an error when the native handle or X11 connection is unavailable.
#[cfg(target_os = "linux")]
pub(crate) fn initialize(window: &Window) -> Result<(), Box<dyn std::error::Error>> {
    let handle = HasWindowHandle::window_handle(window)?;
    let id = match handle.as_raw() {
        RawWindowHandle::Xcb(handle) => handle.window.get(),
        RawWindowHandle::Xlib(handle) => u32::try_from(handle.window)?,
        _ => return Ok(()),
    };
    let x11 = X11::new(id)?;
    x11.hide_window()?;
    let _ = X11_INSTANCE.set(x11);
    Ok(())
}

/// Preserve the compositor focus request associated with a global shortcut.
/// Other activation sources retain their existing native activation behavior.
pub(crate) fn activate_from_hotkey(handle: WindowHandle<Root>, cx: &AsyncApp) {
    cx.update(|cx| {
        let _ = handle.update(cx, |_, window, cx| {
            window.dispatch_action(Box::new(super::board::Active), cx);
        });
    });
    #[cfg(target_os = "linux")]
    if crate::linux::hyprland::is_session() {
        cx.background_executor()
            .spawn(async {
                crate::linux::hyprland::focus_popup();
            })
            .detach();
    }
}

/// Hide (without destroying) the active window using the platform-native
/// path so subsequent `active_window` calls can restore it.
#[expect(unused_variables, clippy::needless_pass_by_ref_mut)]
pub(crate) fn hide_window<T>(window: &mut Window, cx: &Context<'_, T>, pinned: bool) {
    if pinned {
        return;
    }
    cfg_select! {
        target_os = "macos" => {
            cx.hide();
        }
        target_os = "windows" => {
            if let Ok(window_handle) = HasWindowHandle::window_handle(window)
                && let RawWindowHandle::Win32(win32_handle) = window_handle.as_raw()
            {
                let hwnd = win32_handle.hwnd.get() as *mut std::ffi::c_void;
                // SAFETY: see module-level note. `SW_HIDE` only toggles visibility.
                unsafe {
                    ShowWindow(hwnd, SW_HIDE);
                }
            }
        }
        target_os = "linux" => {
            if let Some(x11) = X11_INSTANCE.get()
                && let Err(e) = x11.hide_window()
            {
                tracing::warn!(error = %e, "failed to hide window");
            }
        }
        _ => {}
    }
}

/// Restore the window and pull it to the foreground, mirroring the user's
/// expectation that activating from the tray / hotkey gives focus.
///
/// `window.activate_window()` is called unconditionally so that a window
/// created hidden (`WindowOptions::show = false`) is also brought on-screen
/// the first time the user triggers the hotkey or the tray "Show" entry —
/// on macOS this maps to `[NSWindow makeKeyAndOrderFront]`, which both
/// orders the window front and gives it key status.
#[expect(clippy::needless_pass_by_ref_mut)]
pub(crate) fn active_window<T>(window: &mut Window, cx: &Context<'_, T>) {
    window.activate_window();

    cfg_select! {
        target_os = "macos" => {
            cx.activate(true);
        }
        target_os = "windows" => {
            if let Ok(window_handle) = HasWindowHandle::window_handle(window)
                && let RawWindowHandle::Win32(win32_handle) = window_handle.as_raw()
            {
                let hwnd = win32_handle.hwnd.get() as *mut std::ffi::c_void;
                // SAFETY: see module-level note. Both calls only affect
                // visibility / focus.
                unsafe {
                    ShowWindow(hwnd, SW_RESTORE);
                    SetForegroundWindow(hwnd);
                }
            }
        }
        target_os = "linux" => {
            if let Some(x11) = X11_INSTANCE.get()
                && let Err(e) = x11.display_and_activate_window()
            {
                tracing::warn!(error = %e, "failed to activate window");
            }
        }
        _ => {}
    }
}

/// Toggle the always-on-top Z-order on platforms that expose it
/// (macOS uses the `AppKit` equivalent at the GPUI level instead).
#[cfg_attr(target_os = "linux", expect(unused_variables))]
#[cfg(not(target_os = "macos"))]
pub fn set_always_on_top(window: &Window, pinned: bool) {
    #[cfg(target_os = "windows")]
    if let Ok(window_handle) = HasWindowHandle::window_handle(window)
        && let RawWindowHandle::Win32(win32_handle) = window_handle.as_raw()
    {
        let hwnd = win32_handle.hwnd.get() as *mut std::ffi::c_void;
        // SAFETY: see module-level note. `SWP_NOMOVE | SWP_NOSIZE` keeps
        // geometry untouched, so this only flips Z-order.
        unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE,
            };
            let hwnd_insert_after: *mut std::ffi::c_void =
                if pinned { HWND_TOPMOST } else { HWND_NOTOPMOST };
            SetWindowPos(hwnd, hwnd_insert_after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        }
    }

    #[cfg(target_os = "linux")]
    if let Some(x11) = X11_INSTANCE.get()
        && let Err(e) = x11.set_always_on_top(pinned)
    {
        tracing::warn!(error = %e, "failed to set always on top");
    }
}

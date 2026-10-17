//! Popup lifetime and native visibility/focus control.
//!
//! One board outlives the native Wayland popup windows. Other platforms keep a
//! resident window; its X11 connection lives for the process lifetime.
//! Win32 handles below are borrowed from a live GPUI window for each call.

use std::cfg_select;

use gpui_kit::{App, AsyncApp, Context, Entity, Global, Window, WindowHandle, component::Root};
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

/// GPUI also reports temporary X11 keyboard grabs as deactivation. Ask the
/// server before dismissing the popup during an OS-managed move gesture.
pub(crate) fn has_native_focus(window: &Window) -> bool {
    if window.is_window_active() {
        return true;
    }
    #[cfg(target_os = "linux")]
    if let Some(x11) = X11_INSTANCE.get() {
        return x11.has_input_focus().unwrap_or_else(|error| {
            tracing::warn!(%error, "failed to query X11 input focus");
            false
        });
    }
    false
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PopupLifetime {
    Resident,
    Transient,
}

struct Popup {
    handle: Option<WindowHandle<Root>>,
    board: Entity<super::board::RopyBoard>,
    lifetime: PopupLifetime,
    #[cfg(target_os = "linux")]
    activation: Option<crate::linux::activation::Activation>,
}

impl Global for Popup {}

impl Popup {
    fn install(
        handle: WindowHandle<Root>,
        lifetime: PopupLifetime,
        show_on_start: bool,
        cx: &mut App,
    ) {
        let Some(board) = handle.read(cx).ok().and_then(|root| {
            root.view()
                .clone()
                .downcast::<super::board::RopyBoard>()
                .ok()
        }) else {
            tracing::error!("failed to retain popup state");
            return;
        };
        cx.set_global(Self {
            handle: Some(handle),
            board,
            lifetime,
            #[cfg(target_os = "linux")]
            activation: None,
        });
        if lifetime == PopupLifetime::Transient {
            cx.set_quit_mode(gpui_kit::QuitMode::Explicit);
            // GPUI applies a one-shot startup token to the first surface. Keep
            // that surface when --show requested a visible first launch.
            if !show_on_start {
                let _ = handle.update(cx, |_, window, _| window.remove_window());
            }
        }
    }
}

/// Retain the board independently of a native window. GPUI's Wayland backend
/// does not hide/remap a surface, so dismissal closes that window instead.
pub(crate) fn install(
    handle: WindowHandle<Root>,
    platform: &std::rc::Rc<dyn gpui_kit::Platform>,
    show_on_start: bool,
    cx: &mut App,
) {
    #[cfg(not(target_os = "linux"))]
    let _ = platform;
    #[cfg(target_os = "linux")]
    let activation = if crate::linux::is_wayland() {
        handle
            .update(cx, |_, window, _| {
                crate::linux::activation::Activation::new(window, platform.clone())
            })
            .ok()
            .and_then(|result| {
                result
                    .map_err(|error| tracing::warn!(%error, "Wayland activation unavailable"))
                    .ok()
            })
    } else {
        None
    };
    let lifetime = cfg_select! {
        target_os = "linux" => {
            if crate::linux::is_wayland() {
                PopupLifetime::Transient
            } else {
                PopupLifetime::Resident
            }
        }
        _ => PopupLifetime::Resident,
    };
    Popup::install(handle, lifetime, show_on_start, cx);
    #[cfg(target_os = "linux")]
    if cx.has_global::<Popup>() {
        cx.global_mut::<Popup>().activation = activation;
    }
}

pub(crate) fn board(cx: &App) -> Option<Entity<super::board::RopyBoard>> {
    cx.try_global::<Popup>().map(|popup| popup.board.clone())
}

pub(crate) fn ensure_window(cx: &mut App) -> Option<WindowHandle<Root>> {
    let popup = cx.try_global::<Popup>()?;
    if let Some(handle) = popup.handle
        && handle.read(cx).is_ok()
    {
        return Some(handle);
    }
    let board = popup.board.clone();
    match super::app::reopen_window(cx, board) {
        Ok(handle) => {
            cx.global_mut::<Popup>().handle = Some(handle);
            Some(handle)
        }
        Err(error) => {
            tracing::error!(%error, "failed to reopen popup");
            None
        }
    }
}

#[cfg(any(test, not(target_os = "linux")))]
pub(crate) fn show(cx: &mut App) {
    show_with_token(None, cx);
}

pub(crate) fn show_with_token(token: Option<&str>, cx: &mut App) {
    #[cfg(not(target_os = "linux"))]
    let _ = token;
    if let Some(handle) = ensure_window(cx) {
        let _ = handle.update(cx, |root, window, cx| {
            if let Ok(board) = root.view().clone().downcast::<super::board::RopyBoard>() {
                board.update(cx, |board, cx| {
                    board.on_active_action(&super::board::Active, window, cx);
                });
            }
            #[cfg(target_os = "linux")]
            if let Some(token) = token
                && crate::linux::is_wayland()
                && let Some(activation) = cx.global::<Popup>().activation.as_ref()
                && let Err(error) = activation.activate(window, token)
            {
                tracing::warn!(%error, "failed to apply Wayland activation token");
            }
        });
    }
}

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
pub(crate) fn activate_from_hotkey(cx: &AsyncApp, token: Option<&str>) {
    cx.update(|cx| show_with_token(token, cx));
    #[cfg(target_os = "linux")]
    if crate::linux::hyprland::is_session() {
        cx.background_executor()
            .spawn(async {
                crate::linux::hyprland::focus_popup();
            })
            .detach();
    }
}

/// Dismiss the active popup. Wayland destroys the native window; resident
/// backends hide it through their platform API.
pub(crate) fn hide_window<T>(window: &mut Window, cx: &Context<'_, T>, pinned: bool) {
    if pinned {
        return;
    }
    if cx
        .try_global::<Popup>()
        .is_some_and(|popup| popup.lifetime == PopupLifetime::Transient)
    {
        window.remove_window();
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
#[cfg_attr(target_os = "linux", expect(unused_variables))]
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

#[cfg(test)]
#[expect(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::{
        assert_matches,
        sync::{Arc, Mutex, RwLock},
    };

    use gpui_kit::{AppContext, KeyBinding, TestAppContext, test::TestWindowExt};

    use super::*;
    use crate::{
        clipboard::CopyTracker,
        config::Settings,
        gui::{board::Hide, repository::GlobalRepository, settings::GlobalSettings},
        repository::ClipboardRecord,
        updater::models::UpdateStatus,
    };

    #[gpui_kit::test]
    fn test_transient_popup_reopens_with_fresh_inputs_and_retained_state(cx: &mut TestAppContext) {
        let records = ["first marker", "second marker"]
            .into_iter()
            .enumerate()
            .map(|(ix, text)| {
                ClipboardRecord::new(
                    ix as u64,
                    text.into(),
                    chrono::Local::now(),
                    crate::repository::ContentType::Text,
                )
            })
            .collect();
        let (copy_tx, _copy_rx) = async_channel::unbounded();
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_global(GlobalSettings::new(Settings::default()));
            cx.set_global(GlobalRepository::new(None));
            cx.bind_keys([KeyBinding::new("escape", Hide, None)]);
            let handle = crate::gui::create_window(
                cx,
                Arc::new(RwLock::new(records)),
                Arc::new(Mutex::new(CopyTracker::default())),
                copy_tx,
            );
            Popup::install(handle, PopupLifetime::Transient, false, cx);
            assert!(cx.windows().is_empty(), "startup must remain hidden");
            board(cx).unwrap().update(cx, |board, _| {
                board.update_manager.status = UpdateStatus::Checking;
            });
        });

        let retained = cx.update(|cx| board(cx).unwrap());
        let mut previous_input = None;
        for query in ["first", "second"] {
            let handle = cx.update(|cx| {
                show(cx);
                ensure_window(cx).expect("visible popup")
            });
            cx.update_window(handle.into(), |_, window, cx| {
                assert_eq!(cx.windows().len(), 1, "only one popup may exist");
                assert_eq!(board(cx).unwrap().entity_id(), retained.entity_id());
                assert_matches!(
                    retained.read(cx).update_manager.status,
                    UpdateStatus::Checking
                );
                let input = retained.read(cx).search_input.entity_id();
                assert_ne!(
                    previous_input,
                    Some(input),
                    "input subscriptions belong to the new window"
                );
                previous_input = Some(input);
                window.render_frame(cx);
                window.press("/", cx);
                window.input(query, cx);
                window.render_frame(cx);
                assert_eq!(retained.read(cx).search_input.read(cx).value(), query);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update_window(handle.into(), |_, window, cx| {
                assert_eq!(retained.read(cx).last_search_query, query);
                assert_eq!(retained.read(cx).filtered_record_indices.len(), 1);
                // First Escape returns from search to the list, second dismisses.
                window.press("escape", cx);
                window.press("escape", cx);
            })
            .unwrap();
            cx.run_until_parked();
            cx.update(|cx| assert_eq!(cx.windows().len(), 0));
        }
        cx.update(|cx| {
            let handle = ensure_window(cx).unwrap();
            let input = retained.read(cx).search_input.entity_id();
            Popup::install(handle, PopupLifetime::Transient, true, cx);
            show(cx);
            assert_eq!(
                retained.read(cx).search_input.entity_id(),
                input,
                "visible startup must keep the surface that consumed the startup token"
            );
            crate::gui::tray::send_open_settings(cx);
            assert_eq!(cx.windows().len(), 1);
            assert_matches!(
                retained.read(cx).active_panel,
                crate::gui::board::ActivePanel::Settings
            );
            crate::gui::tray::send_open_about(cx);
            assert_eq!(cx.windows().len(), 1);
            assert_matches!(
                retained.read(cx).active_panel,
                crate::gui::board::ActivePanel::About
            );
        });
    }
}

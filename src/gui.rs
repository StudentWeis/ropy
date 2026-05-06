/// Window creation, asset loading, and theme application.
pub mod app;
/// Main clipboard board UI and interactions.
pub mod board;
mod constants;
/// Global hotkey registration and updates.
pub mod hotkey;
/// Secondary panels embedded in the board UI.
pub mod panel;
mod paste;
/// Theme identifiers and bundled theme loading.
pub mod theme;
/// Tray icon lifecycle and menu events.
pub mod tray;
/// Shared GUI-only platform utilities.
pub mod utils;

#[cfg(target_os = "linux")]
/// Linux X11 integration hooks.
pub mod x11;

pub(crate) use app::{Assets, create_window};
pub(crate) use tray::start_tray_handler;
#[cfg(target_os = "macos")]
pub(crate) use utils::set_activation_policy_accessory;
pub(crate) use utils::{
    active_window, hide_window, reset_window_geometry_for_activation, surface_with_opacity,
};

use gpui::{Context, Window};

/// Hide the window based on the platform
pub fn hide_window<T>(_window: &mut Window, _cx: &mut Context<T>) {
    #[cfg(target_os = "windows")]
    _window.minimize_window();
    #[cfg(target_os = "macos")]
    _cx.hide();
}

/// Activate the window based on the platform
pub fn active_window<T>(_window: &mut Window, _cx: &mut Context<T>) {
    #[cfg(target_os = "windows")]
    _window.activate_window();
    #[cfg(target_os = "macos")]
    _cx.activate(true);
}

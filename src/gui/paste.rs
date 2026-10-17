use std::{cfg_select, thread, time::Duration};

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use thiserror::Error;

const PASTE_DELAY_MS: u64 = 50;

#[cfg(target_os = "linux")]
fn input_settings(wayland: bool) -> Settings {
    // Enigo 0.6 sends each key to every connected backend. It has no backend
    // selector, so give the unused backend an explicitly invalid endpoint.
    // "disabled" fails X11 display parsing before any connection attempt;
    // /dev/null cannot be a Wayland socket. Do not change process environment.
    if wayland {
        Settings {
            x11_display: Some("disabled".into()),
            ..Settings::default()
        }
    } else {
        Settings {
            wayland_display: Some("/dev/null".into()),
            ..Settings::default()
        }
    }
}

#[derive(Debug, Error)]
pub(super) enum PasteError {
    #[error("failed to initialize input simulator: {0}")]
    Initialize(String),
    #[error("failed to send paste shortcut: {0}")]
    Shortcut(String),
}

pub(super) fn trigger_paste() -> Result<(), PasteError> {
    // Give the previous application a brief moment to regain focus after the popup hides.
    thread::sleep(paste_delay());

    let settings = cfg_select! {
        target_os = "linux" => input_settings(crate::linux::is_wayland()),
        _ => Settings::default(),
    };
    let mut enigo =
        Enigo::new(&settings).map_err(|error| PasteError::Initialize(error.to_string()))?;
    let modifier = paste_modifier_key();

    enigo
        .key(modifier, Direction::Press)
        .map_err(|error| PasteError::Shortcut(error.to_string()))?;

    let paste_result = enigo
        .key(Key::Unicode('v'), Direction::Click)
        .map_err(|error| PasteError::Shortcut(error.to_string()));
    let release_result = enigo
        .key(modifier, Direction::Release)
        .map_err(|error| PasteError::Shortcut(error.to_string()));

    paste_result?;
    release_result?;
    Ok(())
}

const fn paste_delay() -> Duration {
    Duration::from_millis(PASTE_DELAY_MS)
}

const fn paste_modifier_key() -> Key {
    cfg_select! {
        target_os = "macos" => Key::Meta,
        _ => Key::Control,
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;

    #[cfg(target_os = "linux")]
    #[test]
    fn test_input_settings_wayland_prevents_duplicate_x11_input() {
        let settings = input_settings(true);
        assert!(settings.x11_display.is_some_and(|display| {
            x11rb::rust_connection::RustConnection::connect(Some(&display)).is_err()
        }));
        assert!(settings.wayland_display.is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_input_settings_x11_disables_wayland_connection() {
        let settings = input_settings(false);
        assert!(settings.x11_display.is_none());
        assert_eq!(settings.wayland_display.as_deref(), Some("/dev/null"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_paste_modifier_key_when_macos_returns_meta() {
        assert_matches!(paste_modifier_key(), Key::Meta);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_paste_modifier_key_when_non_macos_returns_control() {
        assert_matches!(paste_modifier_key(), Key::Control);
    }
}

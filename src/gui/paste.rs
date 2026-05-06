use std::{cfg_select, thread, time::Duration};

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use thiserror::Error;

const PASTE_DELAY_MS: u64 = 50;

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

    let mut enigo = Enigo::new(&Settings::default())
        .map_err(|error| PasteError::Initialize(error.to_string()))?;
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
        target_os = "macos" => { Key::Meta },
        _ => { Key::Control },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paste_delay_when_called_returns_expected_duration() {
        assert_eq!(paste_delay(), Duration::from_millis(PASTE_DELAY_MS));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_paste_modifier_key_when_macos_returns_meta() {
        assert!(matches!(paste_modifier_key(), Key::Meta));
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_paste_modifier_key_when_non_macos_returns_control() {
        assert!(matches!(paste_modifier_key(), Key::Control));
    }
}

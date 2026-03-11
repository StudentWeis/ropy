use std::{thread, time::Duration};

use enigo::{Direction, Enigo, Key, Keyboard, Settings};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PasteError {
    #[error("failed to initialize input simulator: {0}")]
    Initialize(String),
    #[error("failed to send paste shortcut: {0}")]
    Shortcut(String),
}

pub fn trigger_paste() -> Result<(), PasteError> {
    // Give the previous application a brief moment to regain focus after the popup hides.
    thread::sleep(Duration::from_millis(50));

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

const fn paste_modifier_key() -> Key {
    #[cfg(target_os = "macos")]
    return Key::Meta;
    #[cfg(not(target_os = "macos"))]
    return Key::Control;
}

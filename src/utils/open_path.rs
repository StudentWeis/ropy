//! Helpers for opening filesystem paths with the host's default file manager.

use std::{path::Path, process::Command};

/// Reveal `path` in the host operating system's default file manager.
///
/// On unsupported platforms this is a no-op; on supported platforms any
/// failure to spawn the opener is intentionally swallowed because the caller
/// (a UI button) has no meaningful way to surface it.
pub fn open_in_file_manager(path: &Path) {
    let opener = cfg_select! {
        target_os = "macos" => "open",
        target_os = "windows" => "explorer",
        target_os = "linux" => "xdg-open",
        _ => return,
    };
    let _ = Command::new(opener).arg(path).spawn();
}

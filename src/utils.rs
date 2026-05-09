/// Clipboard file-list serialization and normalization helpers.
pub mod clipboard_files;
/// Content hashing helpers.
pub mod hash;
/// Logging initialization and file locations.
pub mod logging;
#[cfg(target_os = "windows")]
/// Windows single-instance enforcement.
pub mod single_instance;
/// Lock helpers that recover from poisoning.
pub mod sync;

use std::{path::Path, process::Command};

pub(crate) use clipboard_files::{
    deserialize_file_paths, hash_file_paths, normalize_file_paths, serialize_file_paths,
};
pub(crate) use hash::content_hash;
pub(crate) use logging::init as init_logging;
#[cfg(target_os = "windows")]
pub use single_instance::ensure_single_instance;
pub(crate) use sync::{lock_or_recover, read_or_recover, write_or_recover};

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

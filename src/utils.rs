/// Logging initialization and file locations.
pub mod logging;
#[cfg(target_os = "windows")]
/// Windows single-instance enforcement.
pub mod single_instance;
/// Lock helpers that recover from poisoning.
pub mod sync;

use std::{
    path::Path,
    process::{Command, ExitStatus},
    thread::{self, JoinHandle},
};

pub(crate) use logging::init as init_logging;
#[cfg(target_os = "windows")]
pub use single_instance::ensure_single_instance;
pub(crate) use sync::{lock_or_recover, read_or_recover, write_or_recover};

/// Replace `path` with `bytes`, staging the write next to the destination so a
/// crash leaves either the previous file or the complete new one.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;

    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("path has no parent directory"))?;
    std::fs::create_dir_all(parent)?;
    let mut staging = tempfile::NamedTempFile::new_in(parent)?;
    staging.write_all(bytes)?;
    staging.as_file().sync_all()?;
    staging.persist(path).map_err(|error| error.error)?;
    Ok(())
}

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
    let mut command = Command::new(opener);
    command.arg(path);
    let _ = spawn_opener(command);
}

fn spawn_opener(mut command: Command) -> std::io::Result<JoinHandle<std::io::Result<ExitStatus>>> {
    // Start the waiter first: failure to create the thread cannot leave an
    // already-started child without an owner to reap it.
    thread::Builder::new()
        .name("file-manager-opener".into())
        .spawn(move || command.status())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(any(unix, target_os = "windows"))]
    #[expect(clippy::unwrap_used)]
    fn test_spawn_opener_waits_for_child_exit() {
        let mut command = cfg_select! {
            unix => Command::new("sh"),
            target_os = "windows" => Command::new("cmd"),
        };
        command.args(cfg_select! {
            unix => ["-c", "exit 7"],
            target_os = "windows" => ["/C", "exit /B 7"],
        });
        let status = spawn_opener(command).unwrap().join().unwrap().unwrap();
        assert_eq!(status.code(), Some(7));
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_spawn_opener_missing_command_returns_error_without_panicking() {
        let directory = tempfile::tempdir().unwrap();
        let command = Command::new(directory.path().join("missing-opener"));
        let result = spawn_opener(command).unwrap().join().unwrap();
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::NotFound);
    }
}

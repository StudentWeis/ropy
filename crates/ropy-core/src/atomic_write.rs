//! Durable replacement of persisted state files.

use std::{fs::File, path::Path};

/// Replace `path` with the bytes produced by `write`.
///
/// The staging file lives in the destination directory, so the final rename
/// stays on one filesystem and a crash leaves either the previous file or the
/// complete new one.
#[expect(
    clippy::redundant_pub_crate,
    reason = "crate-wide helper behind a private module"
)]
pub(crate) fn write_atomic<E>(
    path: &Path,
    write: impl FnOnce(&mut File) -> Result<(), E>,
) -> Result<(), E>
where
    E: From<std::io::Error>,
{
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("path has no parent directory"))?;
    std::fs::create_dir_all(parent)?;
    let mut staging = tempfile::NamedTempFile::new_in(parent)?;
    write(staging.as_file_mut())?;
    staging.as_file().sync_all()?;
    staging.persist(path).map_err(|error| error.error)?;
    Ok(())
}

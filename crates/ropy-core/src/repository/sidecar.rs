//! Filesystem side-effects for clipboard records.
//!
//! `repo.rs` owns database/index transactions; `assets.rs` owns payload reads,
//! writes and thumbnail paths. This module composes best-effort removal for a
//! record, superseded payload or whole repository. Callers hold the repository
//! operation lock when cleanup is part of a mutation.

use std::{fs, path::Path};

use super::{
    assets::{remove_rich_text_files, thumb_path_for},
    backend::StorageBackend,
    errors::RepositoryError,
    models::{ClipboardRecord, ContentType, RichTextMeta},
    repo::ClipboardRepository,
};

/// Publish the complete ownership marker atomically, including on a racing first open.
pub(super) fn claim_image_directory(
    directory: &Path,
    database: &Path,
) -> Result<(), RepositoryError> {
    use std::io::{ErrorKind, Write as _};
    fs::create_dir_all(directory)?;
    let directory = directory.canonicalize()?;
    let marker = directory.join(".ropy-database");
    // Keep a database and its sibling image directory relocatable together.
    let owner = directory
        .parent()
        .and_then(|parent| database.strip_prefix(parent).ok())
        .unwrap_or(database)
        .as_os_str()
        .as_encoded_bytes();
    match fs::read(&marker) {
        Ok(bytes) if bytes == owner => return Ok(()),
        Ok(_) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {
            let mut staged = tempfile::NamedTempFile::new_in(&directory)?;
            staged.write_all(owner)?;
            staged.as_file().sync_all()?;
            match staged.persist_noclobber(&marker) {
                Ok(_) => return Ok(()),
                Err(error) if error.error.kind() == ErrorKind::AlreadyExists => {
                    if fs::read(&marker)? == owner {
                        return Ok(());
                    }
                }
                Err(error) => return Err(error.error.into()),
            }
        }
        Err(error) => return Err(error.into()),
    }
    Err(RepositoryError::DatabaseOpen(format!(
        "image directory {} belongs to another database",
        directory.display()
    )))
}

/// Remove an image file together with its generated thumbnail.
///
/// Both removals are best-effort: missing files are ignored because callers
/// (delete / dedup) treat sidecar cleanup as advisory.
pub(super) fn remove_image_files(path: &str) {
    let _ = fs::remove_file(path);
    let thumb_path = thumb_path_for(Path::new(path));
    let _ = fs::remove_file(thumb_path);
}

impl<B: StorageBackend> ClipboardRepository<B> {
    pub(super) fn remove_owned_image(&self, path: &str) {
        if self.backend.database_path().is_none()
            || Path::new(path).parent() == Some(self.images_dir())
        {
            remove_image_files(path);
        }
    }

    /// Old records can still point into a different database's image directory.
    pub(super) fn remove_record_sidecars(&self, record: &ClipboardRecord) {
        if record.content_type == ContentType::Image {
            self.remove_owned_image(&record.content);
        }
        if let Some(meta) = record.rich_text_meta.as_ref() {
            remove_rich_text_files(meta);
        }
    }
}

/// When a rich-text record is overwritten, delete any previous HTML/RTF files
/// that the new metadata no longer references.
pub(super) fn remove_superseded_rich_text_files(
    previous: Option<&RichTextMeta>,
    next: &RichTextMeta,
) {
    let Some(previous) = previous else {
        return;
    };

    if let Some(path) = previous.html_path.as_deref()
        && previous.html_path != next.html_path
    {
        let _ = fs::remove_file(path);
    }
    if let Some(path) = previous.rtf_path.as_deref()
        && previous.rtf_path != next.rtf_path
    {
        let _ = fs::remove_file(path);
    }
}

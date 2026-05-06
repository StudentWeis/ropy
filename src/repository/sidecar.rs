//! Filesystem side-effects for clipboard records.
//!
//! `repo.rs` owns the database (postcard + redb) encode/decode. Anything that
//! actually touches the filesystem — image files, image thumbnails, rich-text
//! HTML/RTF sidecar files, and the per-record directories that hold them —
//! lives here as plain free functions so it can be reasoned about and tested
//! independently of the repository struct.

use std::{fs, path::Path};

use super::models::{ClipboardRecord, ContentType, RichTextMeta};
use crate::clipboard::{remove_rich_text_files, thumb_path_for};

/// Remove an image file together with its generated thumbnail.
///
/// Both removals are best-effort: missing files are ignored because callers
/// (delete / dedup) treat sidecar cleanup as advisory.
pub(super) fn remove_image_files(path: &str) {
    let _ = fs::remove_file(path);
    let thumb_path = thumb_path_for(Path::new(path));
    let _ = fs::remove_file(thumb_path);
}

/// Remove every filesystem artifact that belongs to a single record.
///
/// Currently that means image + thumbnail for `ContentType::Image`, and the
/// HTML/RTF sidecar files referenced by `rich_text_meta` for `ContentType::RichText`.
pub(super) fn remove_record_sidecars(record: &ClipboardRecord) {
    if record.content_type == ContentType::Image {
        remove_image_files(&record.content);
    }

    if let Some(meta) = record.rich_text_meta.as_ref() {
        remove_rich_text_files(meta);
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

/// Recursively remove the images and rich-text directories.
///
/// Used by both schema-migration cleanup and `ClipboardRepository::clear`.
/// Failures are logged at warn level rather than swallowed silently, so
/// orphaned files never disappear without a trace.
pub(super) fn purge_sidecar_dirs(images_dir: &Path, rich_text_dir: &Path) {
    if images_dir.exists()
        && let Err(error) = fs::remove_dir_all(images_dir)
    {
        tracing::warn!(error = %error, path = %images_dir.display(), "failed to remove images dir");
    }
    if rich_text_dir.exists()
        && let Err(error) = fs::remove_dir_all(rich_text_dir)
    {
        tracing::warn!(error = %error, path = %rich_text_dir.display(), "failed to remove rich-text dir");
    }
}

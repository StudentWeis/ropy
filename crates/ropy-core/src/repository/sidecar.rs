//! Filesystem side-effects for clipboard records.
//!
//! `repo.rs` owns database/index transactions; `assets.rs` owns payload reads,
//! writes and thumbnail paths. This module composes best-effort removal for a
//! record, superseded payload or whole repository. Callers hold the repository
//! operation lock when cleanup is part of a mutation.

use std::{fs, path::Path};

use super::{
    assets::{remove_rich_text_files, thumb_path_for},
    models::{ClipboardRecord, ContentType, RichTextMeta},
};

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

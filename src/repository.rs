/// Clipboard payload files and thumbnails.
pub mod assets;
/// Storage backend traits and implementations (memory, redb).
pub mod backend;
mod cleanup;
/// File-list encoding and normalization.
pub mod clipboard_files;
mod display;
/// Repository error types.
pub mod errors;
mod favorites;
/// Stable content identity.
pub mod hash;
/// Persisted clipboard record models.
pub mod models;
/// Repository entry points and persistence APIs.
pub mod repo;
mod sidecar;
#[cfg(test)]
mod test_helpers;
#[cfg(test)]
mod tests;
mod time_index;

pub(crate) use clipboard_files::{
    deserialize_file_paths, hash_file_paths, normalize_file_paths, serialize_file_paths,
};
pub(crate) use hash::content_hash;
pub(crate) use models::{ClipboardRecord, ContentType, RichTextMeta, SharedRecords};
pub(crate) use repo::ClipboardRepository;

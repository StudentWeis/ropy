/// Clipboard payload files and thumbnails.
pub mod assets;
/// Storage backend traits and implementations (memory, redb).
pub mod backend;
mod cleanup;
/// File-list encoding and normalization.
mod clipboard_files;
mod display;
/// Repository error types.
pub mod errors;
mod favorites;
/// Stable content identity.
mod hash;
/// Persisted clipboard record models.
mod models;
/// Repository entry points and persistence APIs.
mod repo;
mod sidecar;
#[cfg(test)]
mod test_helpers;
#[cfg(test)]
mod tests;
mod time_index;

pub use clipboard_files::{
    deserialize_file_paths, hash_file_paths, normalize_file_paths, serialize_file_paths,
};
pub use hash::content_hash;
pub use models::{ClipboardRecord, ContentType, RichTextMeta};
pub use repo::ClipboardRepository;

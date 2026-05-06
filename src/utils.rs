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

pub(crate) use clipboard_files::{
    deserialize_file_paths, hash_file_paths, normalize_file_paths, serialize_file_paths,
};
pub(crate) use hash::content_hash;
pub(crate) use logging::init as init_logging;
#[cfg(target_os = "windows")]
pub use single_instance::ensure_single_instance;
pub(crate) use sync::{lock_or_recover, read_or_recover, write_or_recover};

mod clipboard_files;
mod hash;
pub mod logging;
mod profiling;
#[cfg(target_os = "windows")]
mod single_instance;
mod sync;

pub use clipboard_files::{
    deserialize_file_paths, hash_file_paths, normalize_file_paths, serialize_file_paths,
};
pub use hash::content_hash;
pub use logging::init as init_logging;
pub use profiling::{finish_heap_profiling, start_heap_profiling};
#[cfg(target_os = "windows")]
pub use single_instance::ensure_single_instance;
pub use sync::{lock_or_recover, read_or_recover, write_or_recover};

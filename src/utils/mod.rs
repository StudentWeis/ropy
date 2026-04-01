mod hash;
pub mod logging;
#[cfg(target_os = "windows")]
mod single_instance;
mod sync;

pub use hash::content_hash;
pub use logging::init as init_logging;
#[cfg(target_os = "windows")]
pub use single_instance::ensure_single_instance;
pub use sync::{lock_or_recover, read_or_recover, write_or_recover};

mod hash;
pub mod logging;
#[cfg(target_os = "windows")]
mod single_instance;

pub use hash::content_hash;
pub use logging::init as init_logging;
#[cfg(target_os = "windows")]
pub use single_instance::ensure_single_instance;

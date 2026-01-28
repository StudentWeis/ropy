#[cfg(target_os = "windows")]
mod single_instance;

#[cfg(target_os = "windows")]
pub use single_instance::ensure_single_instance;

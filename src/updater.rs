//! Auto-update module – check for new releases and install updates.

/// Release discovery and version comparison.
pub mod checker;
/// Update archive download and installation.
pub mod downloader;
/// Update-related error types.
pub mod errors;
/// HTTP and curl invocation helpers for updates.
pub mod http;
/// Release and update state models.
pub mod models;

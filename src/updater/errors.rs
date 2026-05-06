//! Auto-update error types

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum UpdateError {
    #[error("network request failed: {0}")]
    Network(String),

    #[error("failed to parse release info: {0}")]
    Parse(String),

    #[error("no compatible asset found for target: {0}")]
    NoCompatibleAsset(String),

    #[error("checksum verification failed (expected {expected}, got {actual})")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("failed to extract archive: {0}")]
    Extract(String),

    #[error("failed to replace binary: {0}")]
    Replace(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

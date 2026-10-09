//! Auto-update error types

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum UpdateError {
    #[error("this installation is managed externally")]
    ManagedInstallation,

    #[error("failed to restart application: {0}")]
    Restart(String),

    #[error("network request failed: {0}")]
    Network(String),

    #[error("GitHub API rate limit exceeded; please try again later")]
    RateLimited,

    #[error("failed to parse release info: {0}")]
    Parse(String),

    #[error("no compatible asset found for target: {0}")]
    NoCompatibleAsset(String),

    #[error("release is missing checksum asset: {0}")]
    MissingChecksumAsset(String),

    #[error("invalid checksum file: {0}")]
    InvalidChecksum(String),

    #[error("checksum verification failed (expected {expected}, got {actual})")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("failed to extract archive: {0}")]
    Extract(String),

    #[error("failed to replace binary: {0}")]
    Replace(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UpdateFailure {
    Network,
    RateLimited,
    Verification,
    Installation,
    Restart,
    Managed,
    Release,
}

impl From<&UpdateError> for UpdateFailure {
    fn from(error: &UpdateError) -> Self {
        match error {
            UpdateError::Network(_) => Self::Network,
            UpdateError::RateLimited => Self::RateLimited,
            UpdateError::ChecksumMismatch { .. }
            | UpdateError::InvalidChecksum(_)
            | UpdateError::MissingChecksumAsset(_) => Self::Verification,
            UpdateError::Extract(_) | UpdateError::Replace(_) | UpdateError::Io(_) => {
                Self::Installation
            }
            UpdateError::Restart(_) => Self::Restart,
            UpdateError::ManagedInstallation => Self::Managed,
            UpdateError::Parse(_) | UpdateError::NoCompatibleAsset(_) => Self::Release,
        }
    }
}

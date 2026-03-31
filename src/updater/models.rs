//! Data models for auto-update

use serde::Deserialize;

/// A GitHub Release as returned by the API
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubRelease {
    pub tag_name: String,
    pub body: Option<String>,
    pub assets: Vec<GitHubAsset>,
}

/// A single asset attached to a GitHub Release
#[derive(Debug, Clone, Deserialize)]
pub struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// Parsed release information ready for display / download
#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseInfo {
    pub version: String,
    pub release_notes: String,
    pub download_url: String,
    pub checksum_url: String,
    pub asset_size: u64,
}

/// Current state of the update lifecycle, shared with the UI
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStatus {
    /// No check has been performed yet
    Idle,
    /// A version check is in progress
    Checking,
    /// A newer version is available
    Available(ReleaseInfo),
    /// The running version is the latest
    UpToDate,
    /// Downloading the new binary (progress 0.0 – 1.0)
    Downloading(f32),
    /// The new binary has been written; a restart is needed
    ReadyToRestart,
    /// An error occurred
    Error(String),
}

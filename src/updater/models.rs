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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_github_release_deserialize_when_valid_json_parses_assets() {
        let release: GitHubRelease = serde_json::from_str(
            r#"{
                "tag_name": "v0.4.2",
                "body": "Bug fixes",
                "assets": [
                    {
                        "name": "ropy-aarch64-apple-darwin.tar.xz",
                        "browser_download_url": "https://example.com/ropy.tar.xz",
                        "size": 1024
                    }
                ]
            }"#,
        )
        .unwrap();

        assert_eq!(release.tag_name, "v0.4.2");
        assert_eq!(release.body.as_deref(), Some("Bug fixes"));
        assert_eq!(release.assets.len(), 1);
        assert_eq!(release.assets[0].name, "ropy-aarch64-apple-darwin.tar.xz");
        assert_eq!(release.assets[0].size, 1024);
    }

    #[test]
    fn test_update_status_available_when_cloned_preserves_release_info() {
        let release = ReleaseInfo {
            version: "0.4.2".to_string(),
            release_notes: "Bug fixes".to_string(),
            download_url: "https://example.com/ropy.tar.xz".to_string(),
            checksum_url: "https://example.com/ropy.tar.xz.sha256".to_string(),
            asset_size: 1024,
        };

        let status = UpdateStatus::Available(release.clone());

        assert_eq!(status, UpdateStatus::Available(release));
    }

    #[test]
    fn test_update_status_error_when_compared_tracks_message() {
        let left = UpdateStatus::Error("network failed".to_string());
        let right = UpdateStatus::Error("network failed".to_string());

        assert_eq!(left, right);
    }
}

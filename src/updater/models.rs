//! Data models for auto-update.

use serde::Deserialize;

/// Subset of GitHub's Release JSON we actually consume — extra fields are
/// ignored so upstream additions can't break deserialization.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GitHubRelease {
    pub tag_name: String,
    pub body: Option<String>,
    pub assets: Vec<GitHubAsset>,
    #[serde(default)]
    pub draft: bool,
    #[serde(default)]
    pub prerelease: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct GitHubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

/// Release information already resolved to the current target triple, ready
/// for the UI and the downloader without re-parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReleaseInfo {
    pub version: String,
    pub release_notes: String,
    pub download_url: String,
    pub checksum_url: String,
    pub asset_size: u64,
}

/// Update lifecycle state owned by the board update controller. Variants are
/// observable, hence `PartialEq` for cheap change detection.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum UpdateStatus {
    Idle,
    Checking,
    Available(ReleaseInfo),
    UpToDate,
    /// Download progress in `0.0..=1.0`.
    Downloading(f32),
    Verifying,
    Extracting,
    Staging,
    Restarting,
    /// A verified payload is staged; restart installs it and confirms startup.
    ReadyToRestart,
    RolledBack,
    Error(super::errors::UpdateFailure),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[expect(clippy::unwrap_used)]
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
}

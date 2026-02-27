//! Version checker – queries GitHub Releases API and compares versions.

use semver::Version;

use super::{
    errors::UpdateError,
    models::{GitHubRelease, ReleaseInfo},
};

/// GitHub repository coordinates
const REPO_OWNER: &str = "StudentWeis";
const REPO_NAME: &str = "ropy";

/// Build target triple, injected by `build.rs`
const TARGET: &str = env!("TARGET");

/// Current crate version from `Cargo.toml`
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Return the expected asset filename for the given target triple.
fn expected_asset_name(target: &str) -> String {
    let ext = if target.contains("windows") {
        "zip"
    } else {
        "tar.xz"
    };
    format!("ropy-{target}.{ext}")
}

/// Check for a newer release on GitHub.
///
/// Returns `Ok(Some(ReleaseInfo))` when a newer version is found,
/// `Ok(None)` when we are up-to-date, or an `Err` on failure.
pub fn check_for_update(include_prerelease: bool) -> Result<Option<ReleaseInfo>, UpdateError> {
    let release = fetch_latest_release(include_prerelease)?;
    let latest_version = parse_version(&release.tag_name)?;
    let current_version =
        Version::parse(CURRENT_VERSION).map_err(|e| UpdateError::Parse(e.to_string()))?;

    if latest_version <= current_version {
        return Ok(None);
    }

    let asset_name = expected_asset_name(TARGET);
    let checksum_name = format!("{asset_name}.sha256");

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| UpdateError::NoCompatibleAsset(TARGET.to_string()))?;

    let checksum_asset = release.assets.iter().find(|a| a.name == checksum_name);

    let checksum_url = checksum_asset
        .map(|a| a.browser_download_url.clone())
        .unwrap_or_default();

    Ok(Some(ReleaseInfo {
        version: latest_version.to_string(),
        release_notes: release.body.unwrap_or_default(),
        download_url: asset.browser_download_url.clone(),
        checksum_url,
        asset_size: asset.size,
    }))
}

/// Return the current running version string.
pub const fn current_version() -> &'static str {
    CURRENT_VERSION
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Fetch the latest (non-prerelease by default) release from GitHub.
fn fetch_latest_release(include_prerelease: bool) -> Result<GitHubRelease, UpdateError> {
    if include_prerelease {
        // Need to list releases and pick the first one (which is the latest)
        let url =
            format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases?per_page=10");
        let body = http_get(&url)?;
        let releases: Vec<GitHubRelease> =
            serde_json::from_str(&body).map_err(|e| UpdateError::Parse(e.to_string()))?;
        releases
            .into_iter()
            .next()
            .ok_or_else(|| UpdateError::Parse("no releases found".into()))
    } else {
        let url = format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/latest");
        let body = http_get(&url)?;
        serde_json::from_str(&body).map_err(|e| UpdateError::Parse(e.to_string()))
    }
}

/// Strip an optional `v` / `V` prefix and parse a semver version.
fn parse_version(tag: &str) -> Result<Version, UpdateError> {
    let cleaned = tag
        .strip_prefix('v')
        .or_else(|| tag.strip_prefix('V'))
        .unwrap_or(tag);
    Version::parse(cleaned).map_err(|e| UpdateError::Parse(format!("invalid version '{tag}': {e}")))
}

/// Perform a simple HTTP GET with a JSON `Accept` header and the required
/// GitHub `User-Agent`.
///
/// Uses an external `curl` subprocess to avoid macOS firewall / code-signing
/// restrictions on raw sockets from unsigned `.app` bundles.
fn http_get(url: &str) -> Result<String, UpdateError> {
    let output = std::process::Command::new("curl")
        .args([
            "-sSL",
            "--fail",
            "-H",
            &format!("User-Agent: ropy/{}", env!("CARGO_PKG_VERSION")),
            "-H",
            "Accept: application/vnd.github.v3+json",
            "--connect-timeout",
            "15",
            "--max-time",
            "30",
            url,
        ])
        .output()
        .map_err(|e| {
            tracing::error!(url = %url, error = %e, "failed to launch curl");
            UpdateError::Network(format!("failed to launch curl: {e}"))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(url = %url, status = %output.status, stderr = %stderr, "curl request failed");
        return Err(UpdateError::Network(format!(
            "HTTP request failed (exit {}): {stderr}",
            output.status
        )));
    }

    String::from_utf8(output.stdout).map_err(|e| {
        tracing::error!(url = %url, error = %e, "response body is not valid UTF-8");
        UpdateError::Network(format!("invalid UTF-8 in response: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_parse_version_plain() {
        let v = parse_version("1.2.3").unwrap();
        assert_eq!(v, Version::new(1, 2, 3));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_parse_version_with_v_prefix() {
        let v = parse_version("v0.2.1").unwrap();
        assert_eq!(v, Version::new(0, 2, 1));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_parse_version_prerelease() {
        let v = parse_version("v0.3.0-beta").unwrap();
        assert!(!v.pre.is_empty());
    }

    #[test]
    fn test_expected_asset_name_macos() {
        let name = expected_asset_name("aarch64-apple-darwin");
        assert_eq!(name, "ropy-aarch64-apple-darwin.tar.xz");
    }

    #[test]
    fn test_expected_asset_name_windows() {
        let name = expected_asset_name("x86_64-pc-windows-msvc");
        assert_eq!(name, "ropy-x86_64-pc-windows-msvc.zip");
    }

    #[test]
    fn test_expected_asset_name_linux() {
        let name = expected_asset_name("x86_64-unknown-linux-gnu");
        assert_eq!(name, "ropy-x86_64-unknown-linux-gnu.tar.xz");
    }
}

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
    super::http::CurlCommandBuilder::new(url)
        .header("Accept: application/vnd.github.v3+json")
        .with_api_timeouts()
        .execute_to_string()
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

    // ── parse_version Error Cases ─────────────────────────────────

    #[test]
    fn test_parse_version_empty_string() {
        let result = parse_version("");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_version_whitespace_only() {
        let result = parse_version("   ");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_version_invalid_chars() {
        let result = parse_version("not-a-version");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_version_missing_components() {
        // Only major version
        let result = parse_version("1");
        assert!(result.is_err());

        // Only major.minor
        let result = parse_version("1.2");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_version_too_many_components() {
        let result = parse_version("1.2.3.4.5");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_version_negative_numbers() {
        let result = parse_version("-1.2.3");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_version_gibberish() {
        let invalid_versions = vec![
            "@@@",
            "v",
            "V",
            "version",
            "1.2.3-rc.😀",   // Invalid unicode in pre-release
            "1.2.3+build🔧", // Invalid unicode in build metadata
        ];

        for version in invalid_versions {
            let result = parse_version(version);
            assert!(result.is_err(), "Expected error for: {version}");
        }
    }

    #[test]
    fn test_parse_version_invalid_prerelease_format() {
        // Invalid prerelease identifier
        let result = parse_version("1.2.3-");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_version_invalid_build_metadata() {
        // Valid version with build metadata should parse
        let result = parse_version("1.2.3+build123");
        assert!(result.is_ok());

        // But this is invalid
        let result = parse_version("1.2.3+");
        assert!(result.is_err());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_parse_version_case_insensitive_v_prefix() {
        // Both 'v' and 'V' should work
        let v1 = parse_version("v1.2.3").unwrap();
        let v2 = parse_version("V1.2.3").unwrap();
        assert_eq!(v1, v2);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_parse_version_with_build_metadata() {
        let v = parse_version("1.2.3+build.123").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_parse_version_complex_prerelease() {
        let v = parse_version("1.0.0-alpha.1+build.123").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 0);
        assert_eq!(v.patch, 0);
        assert!(!v.pre.is_empty());
    }

    // ── expected_asset_name Edge Cases ────────────────────────────

    #[test]
    fn test_expected_asset_name_unknown_target() {
        // Unknown target should default to tar.xz
        let name = expected_asset_name("unknown-target-triple");
        assert_eq!(name, "ropy-unknown-target-triple.tar.xz");
    }

    #[test]
    fn test_expected_asset_name_empty_target() {
        let name = expected_asset_name("");
        assert_eq!(name, "ropy-.tar.xz");
    }

    #[test]
    fn test_expected_asset_name_case_sensitive_windows() {
        // The implementation uses contains("windows") which is case-sensitive
        // Lowercase "windows" should match and return .zip
        let name_lower = expected_asset_name("x86_64-pc-windows-msvc");
        assert_eq!(name_lower, "ropy-x86_64-pc-windows-msvc.zip");

        // Uppercase "WINDOWS" does NOT match contains("windows"), so returns .tar.xz
        let name_upper = expected_asset_name("x86_64-pc-WINDOWS-msvc");
        assert_eq!(name_upper, "ropy-x86_64-pc-WINDOWS-msvc.tar.xz");
    }
}

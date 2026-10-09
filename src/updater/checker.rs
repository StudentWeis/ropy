//! Version checker – discovers GitHub releases and compares versions.

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

fn stable_release_manifest_url() -> String {
    format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases/latest/download/latest.json")
}

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
pub(crate) fn check_for_update(
    include_prerelease: bool,
) -> Result<Option<ReleaseInfo>, UpdateError> {
    let releases = fetch_releases(include_prerelease)?;
    let current_version =
        Version::parse(CURRENT_VERSION).map_err(|e| UpdateError::Parse(e.to_string()))?;

    let installation = super::installation::Installation::current()?;
    select_release(
        releases,
        &current_version,
        TARGET,
        include_prerelease,
        installation.is_bundle(),
    )
}

fn resolve_release(
    release: GitHubRelease,
    current_version: &Version,
    target: &str,
) -> Result<Option<ReleaseInfo>, UpdateError> {
    let latest_version = parse_version(&release.tag_name)?;

    if latest_version <= *current_version {
        return Ok(None);
    }

    let asset_name = expected_asset_name(target);
    let checksum_name = format!("{asset_name}.sha256");

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| UpdateError::NoCompatibleAsset(target.to_string()))?;

    let checksum_url = release
        .assets
        .iter()
        .find(|a| a.name == checksum_name)
        .map(|a| a.browser_download_url.clone())
        .ok_or(UpdateError::MissingChecksumAsset(checksum_name))?;

    Ok(Some(ReleaseInfo {
        version: latest_version.to_string(),
        release_notes: release.body.unwrap_or_default(),
        download_url: asset.browser_download_url.clone(),
        checksum_url,
        asset_size: asset.size,
    }))
}

fn select_release(
    releases: Vec<GitHubRelease>,
    current: &Version,
    target: &str,
    prerelease: bool,
    bundle: bool,
) -> Result<Option<ReleaseInfo>, UpdateError> {
    let mut candidates: Vec<_> = releases
        .into_iter()
        .filter_map(|release| {
            let version = parse_version(&release.tag_name).ok()?;
            (!release.draft
                && (prerelease || (!release.prerelease && version.pre.is_empty()))
                && version > *current)
                .then_some((version, release))
        })
        .collect();
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    let asset_target = if bundle {
        format!("{target}-app")
    } else {
        target.to_string()
    };
    let mut incompatible = None;
    for (_, release) in candidates {
        match resolve_release(release, current, &asset_target) {
            Ok(info) => return Ok(info),
            Err(
                error @ (UpdateError::NoCompatibleAsset(_) | UpdateError::MissingChecksumAsset(_)),
            ) => incompatible = Some(error),
            Err(error) => return Err(error),
        }
    }
    incompatible.map_or(Ok(None), Err)
}

/// Fetch the latest release manifest.
fn fetch_releases(include_prerelease: bool) -> Result<Vec<GitHubRelease>, UpdateError> {
    if include_prerelease {
        // GitHub's stable release redirect excludes prereleases, so the
        // opt-in prerelease path still uses the rate-limited API.
        let url =
            format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases?per_page=100");
        let body = http_get(&url)?;
        let releases: Vec<GitHubRelease> =
            serde_json::from_str(&body).map_err(|e| UpdateError::Parse(e.to_string()))?;
        Ok(releases)
    } else {
        let body = http_get(&stable_release_manifest_url())?;
        serde_json::from_str(&body)
            .map(|release| vec![release])
            .map_err(|e| UpdateError::Parse(e.to_string()))
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

    #[expect(clippy::unwrap_used)]
    fn release(tag: &str, target: &str) -> GitHubRelease {
        serde_json::from_value(serde_json::json!({
            "tag_name": tag,
            "assets": [
                {"name": expected_asset_name(target), "size": 42, "browser_download_url": "https://example.com/archive"},
                {"name": format!("{}.sha256", expected_asset_name(target)), "size": 64, "browser_download_url": "https://example.com/checksum"}
            ]
        })).unwrap()
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_select_release_unsorted_list_chooses_highest_compatible_semver() {
        let releases = vec![
            release("1.1.0", "linux"),
            release("9.0.0", "windows"),
            release("1.9.0", "linux"),
            release("1.2.0", "linux"),
        ];
        assert_eq!(
            select_release(releases, &Version::new(1, 0, 0), "linux", true, false)
                .unwrap()
                .unwrap()
                .version,
            "1.9.0"
        );
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_select_release_stable_channel_excludes_drafts_and_prereleases() {
        let mut draft = release("9.0.0", "linux");
        draft.draft = true;
        let releases = vec![
            draft,
            release("2.0.0-beta.1", "linux"),
            release("1.1.0", "linux"),
        ];
        assert_eq!(
            select_release(releases, &Version::new(1, 0, 0), "linux", false, false)
                .unwrap()
                .unwrap()
                .version,
            "1.1.0"
        );
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_select_release_channel_switch_never_downgrades() {
        assert!(
            select_release(
                vec![release("1.0.0", "linux")],
                &Version::parse("2.0.0-beta.1").unwrap(),
                "linux",
                false,
                false
            )
            .unwrap()
            .is_none()
        );
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_select_release_bundle_requires_complete_app_asset() {
        assert!(
            select_release(
                vec![release("2.0.0", "aarch64-apple-darwin")],
                &Version::new(1, 0, 0),
                "aarch64-apple-darwin",
                false,
                true
            )
            .is_err()
        );
        assert!(
            select_release(
                vec![release("2.0.0", "aarch64-apple-darwin-app")],
                &Version::new(1, 0, 0),
                "aarch64-apple-darwin",
                false,
                true
            )
            .unwrap()
            .is_some()
        );
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_parse_version_plain() {
        let v = parse_version("1.2.3").unwrap();
        assert_eq!(v, Version::new(1, 2, 3));
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_parse_version_with_v_prefix() {
        let v = parse_version("v0.2.1").unwrap();
        assert_eq!(v, Version::new(0, 2, 1));
    }

    #[test]
    #[expect(clippy::unwrap_used)]
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

    #[test]
    fn test_stable_manifest_url_avoids_github_api() {
        let url = stable_release_manifest_url();
        assert_eq!(
            url,
            "https://github.com/StudentWeis/ropy/releases/latest/download/latest.json"
        );
        assert!(!url.contains("api.github.com"));
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_resolve_release_manifest_for_matching_target_returns_download_info() {
        let release: GitHubRelease = serde_json::from_str(
            r#"{
                "tag_name": "0.6.0",
                "body": "Rate-limit resistant updates",
                "assets": [
                    {
                        "name": "ropy-aarch64-apple-darwin.tar.xz",
                        "browser_download_url": "https://github.com/StudentWeis/ropy/releases/download/0.6.0/ropy-aarch64-apple-darwin.tar.xz",
                        "size": 4096
                    },
                    {
                        "name": "ropy-aarch64-apple-darwin.tar.xz.sha256",
                        "browser_download_url": "https://github.com/StudentWeis/ropy/releases/download/0.6.0/ropy-aarch64-apple-darwin.tar.xz.sha256",
                        "size": 99
                    }
                ]
            }"#,
        )
        .unwrap();

        let info = resolve_release(release, &Version::new(0, 5, 4), "aarch64-apple-darwin")
            .unwrap()
            .unwrap();

        assert_eq!(info.version, "0.6.0");
        assert_eq!(info.release_notes, "Rate-limit resistant updates");
        assert_eq!(info.asset_size, 4096);
        assert!(info.download_url.ends_with(".tar.xz"));
        assert!(info.checksum_url.ends_with(".tar.xz.sha256"));
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_resolve_release_when_checksum_asset_missing_returns_error() {
        let release: GitHubRelease = serde_json::from_str(
            r#"{
                "tag_name": "0.6.0",
                "assets": [{
                    "name": "ropy-aarch64-apple-darwin.tar.xz",
                    "browser_download_url": "https://example.com/ropy.tar.xz",
                    "size": 4096
                }]
            }"#,
        )
        .unwrap();

        let result = resolve_release(release, &Version::new(0, 5, 4), "aarch64-apple-darwin");

        assert!(matches!(result, Err(UpdateError::MissingChecksumAsset(_))));
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
    #[expect(clippy::unwrap_used)]
    fn test_parse_version_case_insensitive_v_prefix() {
        // Both 'v' and 'V' should work
        let v1 = parse_version("v1.2.3").unwrap();
        let v2 = parse_version("V1.2.3").unwrap();
        assert_eq!(v1, v2);
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_parse_version_with_build_metadata() {
        let v = parse_version("1.2.3+build.123").unwrap();
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    #[expect(clippy::unwrap_used)]
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

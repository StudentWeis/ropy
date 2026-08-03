#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
use std::{env, path::Path};

#[cfg(target_os = "macos")]
use auto_launch::MacOSLaunchMode;
#[cfg(target_os = "windows")]
use auto_launch::WindowsEnableMode;
use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum AutoStartError {
    #[error("Failed to get executable path: {0}")]
    ExecutablePath(String),
    #[error("Failed to initialize auto-launch: {0}")]
    Initialization(String),
    #[error("Failed to enable auto-start: {0}")]
    Enable(String),
    #[error("Failed to disable auto-start: {0}")]
    Disable(String),
    #[error("Failed to check auto-start status: {0}")]
    StatusCheck(String),
}

/// Owns the platform `AutoLaunch` handle. The app always boots hidden into
/// the tray, so no extra CLI flag is needed to differentiate a
/// system-triggered launch from a user-triggered one.
pub(crate) struct AutoStartManager {
    auto_launch: AutoLaunch,
    #[cfg(target_os = "macos")]
    legacy_launch_agent: AutoLaunch,
}

impl AutoStartManager {
    pub(crate) fn new(app_name: &str) -> Result<Self, AutoStartError> {
        let app_path = Self::get_app_path()?;

        let mut builder = AutoLaunchBuilder::new();
        builder.set_app_name(app_name).set_app_path(&app_path);

        #[cfg(target_os = "macos")]
        builder.set_macos_launch_mode(MacOSLaunchMode::AppleScript);

        #[cfg(target_os = "windows")]
        builder.set_windows_enable_mode(WindowsEnableMode::CurrentUser);

        let auto_launch = builder.build().map_err(|e| {
            AutoStartError::Initialization(format!("Failed to build AutoLaunch: {e}"))
        })?;

        Ok(Self {
            auto_launch,
            #[cfg(target_os = "macos")]
            legacy_launch_agent: Self::build_macos_legacy_launch_agent(app_name, &app_path)?,
        })
    }

    /// Resolve the path that `LaunchAgents` / `.desktop` entries / the
    /// Windows registry should execute. macOS uses a Login Item so bundled
    /// builds point at the `.app` directory that System Settings displays.
    /// On Windows we rewrite Scoop's versioned install path back to the
    /// stable `current` junction — see [`normalise_scoop_path`].
    fn get_app_path() -> Result<String, AutoStartError> {
        let exe_path =
            env::current_exe().map_err(|e| AutoStartError::ExecutablePath(e.to_string()))?;
        Self::resolve_app_path(&exe_path)
    }

    fn resolve_app_path(exe_path: &Path) -> Result<String, AutoStartError> {
        #[cfg(target_os = "macos")]
        if let Some(bundle_path) = macos_app_bundle_path(exe_path) {
            return Ok(bundle_path);
        }

        #[cfg(target_os = "windows")]
        {
            let exe_str = exe_path.to_string_lossy();
            if let Some(normalised) = normalise_scoop_path(&exe_str) {
                return Ok(normalised);
            }
        }

        exe_path.to_str().map(ToString::to_string).ok_or_else(|| {
            AutoStartError::ExecutablePath("Path contains invalid UTF-8".to_string())
        })
    }

    #[cfg(target_os = "macos")]
    fn build_macos_legacy_launch_agent(
        app_name: &str,
        app_path: &str,
    ) -> Result<AutoLaunch, AutoStartError> {
        let mut builder = AutoLaunchBuilder::new();
        builder
            .set_app_name(app_name)
            .set_app_path(app_path)
            .set_macos_launch_mode(MacOSLaunchMode::LaunchAgent);
        builder.build().map_err(|e| {
            AutoStartError::Initialization(format!("Failed to build legacy AutoLaunch: {e}"))
        })
    }

    pub(crate) fn enable(&self) -> Result<(), AutoStartError> {
        self.auto_launch
            .enable()
            .map_err(|e| AutoStartError::Enable(e.to_string()))
    }

    pub(crate) fn disable(&self) -> Result<(), AutoStartError> {
        self.auto_launch
            .disable()
            .map_err(|e| AutoStartError::Disable(e.to_string()))
    }

    pub(crate) fn is_enabled(&self) -> Result<bool, AutoStartError> {
        self.auto_launch
            .is_enabled()
            .map_err(|e| AutoStartError::StatusCheck(e.to_string()))
    }

    /// Make the system state match the user's preference, but skip the
    /// platform call when it already matches — repeatedly toggling
    /// Login Items / registry entries is unnecessary churn and (on
    /// some Linux desktops) racy.
    pub(crate) fn sync_state(&self, enabled: bool) -> Result<(), AutoStartError> {
        let current_enabled = self.is_enabled().unwrap_or(false);
        match autostart_sync_action(enabled, current_enabled) {
            AutoStartSyncAction::None => Ok(()),
            AutoStartSyncAction::Enable => self.enable(),
            AutoStartSyncAction::Disable => self.disable(),
        }?;

        #[cfg(target_os = "macos")]
        self.disable_macos_legacy_launch_agent()?;

        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn disable_macos_legacy_launch_agent(&self) -> Result<(), AutoStartError> {
        self.legacy_launch_agent
            .disable()
            .map_err(|e| AutoStartError::Disable(e.to_string()))
    }
}

#[derive(Debug, Eq, PartialEq)]
enum AutoStartSyncAction {
    None,
    Enable,
    Disable,
}

const fn autostart_sync_action(enabled: bool, current_enabled: bool) -> AutoStartSyncAction {
    if enabled {
        if current_enabled {
            AutoStartSyncAction::None
        } else {
            AutoStartSyncAction::Enable
        }
    } else if current_enabled {
        AutoStartSyncAction::Disable
    } else {
        AutoStartSyncAction::None
    }
}

#[cfg(any(target_os = "macos", test))]
fn macos_app_bundle_path(exe_path: &Path) -> Option<String> {
    let exe_str = exe_path.to_string_lossy();
    exe_str.rfind(".app/").map(|app_bundle_idx| {
        // +4 keeps the `.app` suffix in the slice.
        exe_str[..app_bundle_idx + 4].to_string()
    })
}

/// Rewrite a Windows path that points through a Scoop versioned
/// directory (`...\scoop\apps\<name>\<version>\...`) so the version
/// segment becomes `current`. Returns `None` when `path` is not a
/// Scoop install path or already targets `current`.
///
/// `current_exe()` resolves Scoop's `current` junction to the real
/// versioned directory; persisting that resolved path to the autostart
/// registry pins the entry to a specific version, which Scoop then
/// deletes on upgrade. Pointing at `current` lets the entry survive
/// upgrades.
#[cfg(any(target_os = "windows", test))]
fn normalise_scoop_path(path: &str) -> Option<String> {
    const MARKER: &[u8] = br"\scoop\apps\";

    let bytes = path.as_bytes();
    let last_start = bytes.len().checked_sub(MARKER.len())?;
    let apps_idx =
        (0..=last_start).find(|&i| bytes[i..i + MARKER.len()].eq_ignore_ascii_case(MARKER))?;

    // Marker is pure ASCII, so byte indices are valid UTF-8 char
    // boundaries; the same holds for the `\` positions found below.
    let after_apps = apps_idx + MARKER.len();
    let name_end = path[after_apps..].find('\\')?;
    let version_start = after_apps + name_end + 1;
    let version_end = path[version_start..].find('\\')?;
    let version_segment = &path[version_start..version_start + version_end];
    if version_segment.eq_ignore_ascii_case("current") {
        return None;
    }
    Some(format!(
        "{}current{}",
        &path[..version_start],
        &path[version_start + version_end..],
    ))
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    // These tests touch OS-level auto-launch state (macOS Login Items /
    // legacy LaunchAgents, Linux `.desktop` entries, Windows registry). Running them in parallel
    // can race on that shared global state, so they are serialised explicitly
    // while the rest of the suite runs in parallel.

    #[test]
    #[serial(autostart)]
    fn test_autostart_manager_creation() {
        use crate::constants::APP_NAME;
        let manager = AutoStartManager::new(APP_NAME);
        assert!(manager.is_ok());
    }

    #[test]
    #[serial(autostart)]
    #[expect(clippy::unwrap_used)]
    fn test_get_app_path() {
        let path = AutoStartManager::get_app_path();
        assert!(path.is_ok());
        let path_str = path.unwrap();
        assert!(!path_str.is_empty());
    }

    #[test]
    #[cfg(target_os = "macos")]
    #[expect(clippy::unwrap_used)]
    fn test_resolve_app_path_macos_bundle_executable_returns_bundle_path() {
        let executable_path = Path::new("/Applications/Ropy.app/Contents/MacOS/ropy");

        let resolved = AutoStartManager::resolve_app_path(executable_path).unwrap();

        assert_eq!(resolved, "/Applications/Ropy.app");
    }

    #[test]
    fn test_macos_app_bundle_path_with_inner_executable_returns_bundle_path() {
        let executable_path = Path::new("/Applications/Ropy.app/Contents/MacOS/ropy");

        assert_eq!(
            macos_app_bundle_path(executable_path).as_deref(),
            Some("/Applications/Ropy.app"),
        );
    }

    #[test]
    fn test_macos_app_bundle_path_without_bundle_returns_none() {
        assert_eq!(
            macos_app_bundle_path(Path::new("/usr/local/bin/ropy")),
            None
        );
    }

    #[test]
    fn test_autostart_sync_action_enabled_preference_with_disabled_entry_enables() {
        assert_eq!(
            autostart_sync_action(true, false),
            AutoStartSyncAction::Enable,
        );
    }

    #[test]
    fn test_autostart_sync_action_current_enabled_entry_is_noop() {
        assert_eq!(autostart_sync_action(true, true), AutoStartSyncAction::None);
    }

    #[test]
    fn test_autostart_sync_action_disabled_preference_disables_enabled_entry() {
        assert_eq!(
            autostart_sync_action(false, true),
            AutoStartSyncAction::Disable,
        );
    }

    #[test]
    #[serial(autostart)]
    fn test_sync_state() {
        let manager = AutoStartManager::new("RopyTest").expect("Failed to create manager");

        // Try disabling (may fail on some environments); don't make the test brittle
        let _ = manager.sync_state(false);

        // Verify state if possible
        if let Ok(enabled) = manager.is_enabled() {
            assert!(!enabled);
        }
    }

    // Pure-string Scoop normalisation tests — no OS state, safe to run
    // in parallel with each other on any platform.

    #[test]
    fn normalise_scoop_replaces_versioned_segment() {
        assert_eq!(
            normalise_scoop_path(r"C:\Users\foo\scoop\apps\ropy\0.5.1\ropy.exe").as_deref(),
            Some(r"C:\Users\foo\scoop\apps\ropy\current\ropy.exe"),
        );
    }

    #[test]
    fn normalise_scoop_returns_none_for_current_junction() {
        assert_eq!(
            normalise_scoop_path(r"C:\Users\foo\scoop\apps\ropy\current\ropy.exe"),
            None,
        );
    }

    #[test]
    fn normalise_scoop_is_case_insensitive() {
        // Marker matches case-insensitively…
        assert_eq!(
            normalise_scoop_path(r"C:\Users\foo\Scoop\Apps\ropy\0.5.1\ropy.exe").as_deref(),
            Some(r"C:\Users\foo\Scoop\Apps\ropy\current\ropy.exe"),
        );
        // …and so does the `current` check, so we don't pointlessly
        // rewrite an already-stable path.
        assert_eq!(
            normalise_scoop_path(r"C:\Users\foo\scoop\apps\ropy\Current\ropy.exe"),
            None,
        );
    }

    #[test]
    fn normalise_scoop_returns_none_for_non_scoop_path() {
        assert_eq!(
            normalise_scoop_path(r"C:\Program Files\ropy\ropy.exe"),
            None,
        );
        assert_eq!(normalise_scoop_path(""), None);
    }

    #[test]
    fn normalise_scoop_returns_none_when_version_segment_missing() {
        // No `\` after the version, i.e. path ends at the version dir.
        assert_eq!(
            normalise_scoop_path(r"C:\Users\foo\scoop\apps\ropy\0.5.1"),
            None,
        );
        // No app-name segment at all.
        assert_eq!(normalise_scoop_path(r"C:\scoop\apps\"), None);
    }
}

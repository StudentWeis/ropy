#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
use std::{env, path::Path};

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
    app_name: String,
    #[cfg(target_os = "macos")]
    app_path: String,
}

impl AutoStartManager {
    pub(crate) fn new(app_name: &str) -> Result<Self, AutoStartError> {
        let app_path = Self::get_app_path()?;

        let mut builder = AutoLaunchBuilder::new();
        builder.set_app_name(app_name).set_app_path(&app_path);

        #[cfg(target_os = "windows")]
        builder.set_windows_enable_mode(WindowsEnableMode::CurrentUser);

        let auto_launch = builder.build().map_err(|e| {
            AutoStartError::Initialization(format!("Failed to build AutoLaunch: {e}"))
        })?;

        Ok(Self {
            auto_launch,
            #[cfg(target_os = "macos")]
            app_name: app_name.to_owned(),
            #[cfg(target_os = "macos")]
            app_path,
        })
    }

    /// Resolve the path that `LaunchAgents` / `.desktop` entries / the
    /// Windows registry should execute. macOS `LaunchAgent` entries place
    /// this value directly in `ProgramArguments`, so a bundled app must
    /// keep the inner `Contents/MacOS/...` executable path rather than
    /// the `.app` directory. On Windows we rewrite Scoop's versioned
    /// install path back to the stable `current` junction — see
    /// [`normalise_scoop_path`].
    fn get_app_path() -> Result<String, AutoStartError> {
        let exe_path =
            env::current_exe().map_err(|e| AutoStartError::ExecutablePath(e.to_string()))?;
        Self::resolve_app_path(&exe_path)
    }

    fn resolve_app_path(exe_path: &Path) -> Result<String, AutoStartError> {
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
    /// `LaunchAgents` / registry entries is unnecessary churn and (on
    /// some Linux desktops) racy.
    pub(crate) fn sync_state(&self, enabled: bool) -> Result<(), AutoStartError> {
        let current_enabled = self.is_enabled().unwrap_or(false);
        let entry_matches_app_path = if enabled && current_enabled {
            #[cfg(target_os = "macos")]
            {
                self.autostart_entry_matches_app_path()
            }
            #[cfg(not(target_os = "macos"))]
            {
                true
            }
        } else {
            true
        };

        match autostart_sync_action(enabled, current_enabled, entry_matches_app_path) {
            AutoStartSyncAction::None => Ok(()),
            AutoStartSyncAction::Enable => self.enable(),
            AutoStartSyncAction::Disable => self.disable(),
        }
    }

    #[cfg(target_os = "macos")]
    fn autostart_entry_matches_app_path(&self) -> bool {
        let Some(home_dir) = dirs::home_dir() else {
            return false;
        };
        let app_name = &self.app_name;
        let plist_path = home_dir
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{app_name}.plist"));

        std::fs::read_to_string(plist_path)
            .is_ok_and(|contents| launch_agent_contains_app_path(&contents, &self.app_path))
    }
}

#[derive(Debug, Eq, PartialEq)]
enum AutoStartSyncAction {
    None,
    Enable,
    Disable,
}

const fn autostart_sync_action(
    enabled: bool,
    current_enabled: bool,
    entry_matches_app_path: bool,
) -> AutoStartSyncAction {
    if enabled {
        if current_enabled && entry_matches_app_path {
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
fn launch_agent_contains_app_path(contents: &str, app_path: &str) -> bool {
    contents.contains(&format!("<string>{app_path}</string>"))
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

    // These tests touch OS-level auto-launch state (macOS LaunchAgents,
    // Linux `.desktop` entries, Windows registry). Running them in parallel
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
    #[expect(clippy::unwrap_used)]
    fn test_resolve_app_path_macos_bundle_executable_keeps_executable_path() {
        let executable_path = Path::new("/Applications/Ropy.app/Contents/MacOS/ropy");

        let resolved = AutoStartManager::resolve_app_path(executable_path).unwrap();

        assert_eq!(resolved, "/Applications/Ropy.app/Contents/MacOS/ropy");
    }

    #[test]
    fn test_launch_agent_contains_app_path_matching_executable_returns_true() {
        let contents = r"
<key>ProgramArguments</key>
<array><string>/Applications/Ropy.app/Contents/MacOS/ropy</string></array>
";

        assert!(launch_agent_contains_app_path(
            contents,
            "/Applications/Ropy.app/Contents/MacOS/ropy",
        ));
    }

    #[test]
    fn test_launch_agent_contains_app_path_bundle_directory_returns_false() {
        let contents = r"
<key>ProgramArguments</key>
<array><string>/Applications/Ropy.app</string></array>
";

        assert!(!launch_agent_contains_app_path(
            contents,
            "/Applications/Ropy.app/Contents/MacOS/ropy",
        ));
    }

    #[test]
    fn test_autostart_sync_action_stale_enabled_entry_enables() {
        assert_eq!(
            autostart_sync_action(true, true, false),
            AutoStartSyncAction::Enable,
        );
    }

    #[test]
    fn test_autostart_sync_action_current_enabled_entry_is_noop() {
        assert_eq!(
            autostart_sync_action(true, true, true),
            AutoStartSyncAction::None,
        );
    }

    #[test]
    fn test_autostart_sync_action_disabled_preference_disables_enabled_entry() {
        assert_eq!(
            autostart_sync_action(false, true, true),
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

#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
use std::env;

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

        Ok(Self { auto_launch })
    }

    /// Resolve the path that `LaunchAgents` / `.desktop` entries / the
    /// Windows registry should point at. On macOS a release build runs
    /// inside a `.app` bundle and the auto-launch entry must reference
    /// the bundle, not the inner Mach-O — otherwise launchd starts a
    /// detached binary without a working environment.
    ///
    /// On Windows, when installed via Scoop the resolved exe path may
    /// point through a versioned directory (e.g. `apps\ropy\0.5.1\`)
    /// instead of the stable `apps\ropy\current\` junction. We normalise
    /// back to `current` so that the registry entry survives upgrades.
    fn get_app_path() -> Result<String, AutoStartError> {
        let exe_path =
            env::current_exe().map_err(|e| AutoStartError::ExecutablePath(e.to_string()))?;

        #[cfg(target_os = "macos")]
        {
            let exe_str = exe_path.to_string_lossy();
            if let Some(app_bundle_idx) = exe_str.rfind(".app/") {
                // +4 keeps the `.app` suffix in the slice.
                let bundle_path = &exe_str[..app_bundle_idx + 4];
                return Ok(bundle_path.to_string());
            }
        }

        #[cfg(target_os = "windows")]
        {
            let exe_str = exe_path.to_string_lossy();
            // Detect Scoop layout: ...\scoop\apps\<name>\<version>\<binary>
            // and replace the version segment with "current".
            if let Some(apps_idx) = exe_str.to_lowercase().find("\\scoop\\apps\\") {
                let after_apps = apps_idx + "\\scoop\\apps\\".len();
                // Find the app-name segment end (next backslash after apps\)
                if let Some(name_end) = exe_str[after_apps..].find('\\') {
                    let version_start = after_apps + name_end + 1;
                    // Find the version segment end
                    if let Some(version_end) = exe_str[version_start..].find('\\') {
                        let version_segment = &exe_str[version_start..version_start + version_end];
                        if version_segment != "current" {
                            let normalised = format!(
                                "{}current{}",
                                &exe_str[..version_start],
                                &exe_str[version_start + version_end..]
                            );
                            return Ok(normalised);
                        }
                    }
                }
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
        if current_enabled == enabled {
            Ok(())
        } else if enabled {
            self.enable()
        } else {
            self.disable()
        }
    }
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
}

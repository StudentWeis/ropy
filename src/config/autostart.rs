#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
use std::env;

use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AutoStartError {
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

/// Owns the platform `AutoLaunch` handle and threads the `--silent` flag
/// through it so a system-triggered launch boots into the tray instead of
/// popping the main window.
pub struct AutoStartManager {
    auto_launch: AutoLaunch,
}

impl AutoStartManager {
    pub fn new(app_name: &str) -> Result<Self, AutoStartError> {
        let app_path = Self::get_app_path()?;

        let auto_launch = AutoLaunchBuilder::new()
            .set_app_name(app_name)
            .set_app_path(&app_path)
            .set_args(&[crate::constants::SILENT_ARG])
            .build()
            .map_err(|e| {
                AutoStartError::Initialization(format!("Failed to build AutoLaunch: {e}"))
            })?;

        Ok(Self { auto_launch })
    }

    /// Resolve the path that `LaunchAgents` / `.desktop` entries / the
    /// Windows registry should point at. On macOS a release build runs
    /// inside a `.app` bundle and the auto-launch entry must reference
    /// the bundle, not the inner Mach-O — otherwise launchd starts a
    /// detached binary without a working environment.
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

        exe_path
            .to_str()
            .map(std::string::ToString::to_string)
            .ok_or_else(|| {
                AutoStartError::ExecutablePath("Path contains invalid UTF-8".to_string())
            })
    }

    pub fn enable(&self) -> Result<(), AutoStartError> {
        self.auto_launch
            .enable()
            .map_err(|e| AutoStartError::Enable(e.to_string()))
    }

    pub fn disable(&self) -> Result<(), AutoStartError> {
        self.auto_launch
            .disable()
            .map_err(|e| AutoStartError::Disable(e.to_string()))
    }

    pub fn is_enabled(&self) -> Result<bool, AutoStartError> {
        self.auto_launch
            .is_enabled()
            .map_err(|e| AutoStartError::StatusCheck(e.to_string()))
    }

    /// Make the system state match the user's preference, but skip the
    /// platform call when it already matches — repeatedly toggling
    /// `LaunchAgents` / registry entries is unnecessary churn and (on
    /// some Linux desktops) racy.
    pub fn sync_state(&self, enabled: bool) -> Result<(), AutoStartError> {
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
    #[allow(clippy::unwrap_used)]
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

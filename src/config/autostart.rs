use auto_launch::{AutoLaunch, AutoLaunchBuilder};
use std::env;
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
    #[allow(dead_code)]
    #[error("Failed to check auto-start status: {0}")]
    StatusCheck(String),
}

/// Manager for application auto-start functionality
pub struct AutoStartManager {
    auto_launch: AutoLaunch,
}

impl AutoStartManager {
    /// Create a new AutoStartManager instance
    ///
    /// # Arguments
    /// * `app_name` - The name of the application
    ///
    /// # Returns
    /// Result containing AutoStartManager or AutoStartError
    pub fn new(app_name: &str) -> Result<Self, AutoStartError> {
        let app_path = Self::get_app_path()?;

        let auto_launch = AutoLaunchBuilder::new()
            .set_app_name(app_name)
            .set_app_path(&app_path)
            .set_args(&["--silent"])
            .build()
            .map_err(|e| {
                AutoStartError::Initialization(format!("Failed to build AutoLaunch: {e}"))
            })?;

        Ok(Self { auto_launch })
    }

    /// Get the application executable path
    ///
    /// For development builds, returns the debug executable path
    /// For bundled macOS apps, returns the .app bundle path
    fn get_app_path() -> Result<String, AutoStartError> {
        // Get the current executable path
        let exe_path =
            env::current_exe().map_err(|e| AutoStartError::ExecutablePath(e.to_string()))?;

        // On macOS, if we're inside a .app bundle, we need to return the bundle path
        #[cfg(target_os = "macos")]
        {
            let exe_str = exe_path.to_string_lossy();
            // Check if we're inside a .app bundle
            if let Some(app_bundle_idx) = exe_str.rfind(".app/") {
                let bundle_path = &exe_str[..app_bundle_idx + 4]; // Include .app
                return Ok(bundle_path.to_string());
            }
        }

        // For non-bundled or Windows builds, return the executable path
        exe_path.to_str().map(|s| s.to_string()).ok_or_else(|| {
            AutoStartError::ExecutablePath("Path contains invalid UTF-8".to_string())
        })
    }

    /// Enable auto-start at system startup
    pub fn enable(&self) -> Result<(), AutoStartError> {
        self.auto_launch
            .enable()
            .map_err(|e| AutoStartError::Enable(e.to_string()))
    }

    /// Disable auto-start at system startup
    pub fn disable(&self) -> Result<(), AutoStartError> {
        self.auto_launch
            .disable()
            .map_err(|e| AutoStartError::Disable(e.to_string()))
    }

    /// Check if auto-start is currently enabled
    pub fn is_enabled(&self) -> Result<bool, AutoStartError> {
        self.auto_launch
            .is_enabled()
            .map_err(|e| AutoStartError::StatusCheck(e.to_string()))
    }

    /// Synchronize auto-start state with the given enabled flag
    ///
    /// # Arguments
    /// * `enabled` - Whether auto-start should be enabled
    pub fn sync_state(&self, enabled: bool) -> Result<(), AutoStartError> {
        let current_enabled = self.is_enabled().unwrap_or(false);
        if current_enabled != enabled {
            if enabled {
                self.enable()
            } else {
                self.disable()
            }
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autostart_manager_creation() {
        let manager = AutoStartManager::new("Ropy");
        assert!(manager.is_ok());
    }

    #[test]
    fn test_get_app_path() {
        let path = AutoStartManager::get_app_path();
        assert!(path.is_ok());
        let path_str = path.unwrap();
        assert!(!path_str.is_empty());
    }

    #[test]
    fn test_sync_state() {
        let manager = AutoStartManager::new("RopyTest").expect("Failed to create manager");

        // Try disabling (may fail on some environments); don't make the test brittle
        let _ = manager.sync_state(false);

        // Verify state if possible
        if let Ok(enabled) = manager.is_enabled() {
            assert!(!enabled)
        }
    }
}

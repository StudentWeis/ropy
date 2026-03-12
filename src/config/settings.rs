use std::{path::PathBuf, str::FromStr};

use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};

use crate::i18n::Language;

/// Default maximum number of records to display in the UI
const DEFAULT_MAX_HISTORY_RECORDS: usize = 100;
/// Default maximum number of records to store in the repository
const DEFAULT_MAX_STORAGE_RECORDS: usize = 1000;
/// Default interval for update checks (in hours)
const DEFAULT_UPDATE_CHECK_INTERVAL_HOURS: u64 = 24;

/// Application settings structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub hotkey: HotkeySettings,
    pub storage: StorageSettings,
    pub theme: AppTheme,
    pub autostart: AutoStartSettings,
    pub language: Language,
    pub update: UpdateSettings,
    pub preview: PreviewSettings,
    pub confirm: ConfirmSettings,
}

impl Settings {
    /// Get the configuration directory path
    pub fn config_dir() -> Result<PathBuf, ConfigError> {
        dirs::config_dir()
            .map(|dir| dir.join("ropy"))
            .ok_or_else(|| ConfigError::NotFound("Config directory not found".to_string()))
    }

    /// Get the configuration file path
    pub fn config_file() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Load settings from configuration file and environment variables
    pub fn load() -> Result<Self, ConfigError> {
        let config_dir = Self::config_dir()?;
        let config_file = config_dir.join("config");

        // Create config directory if it doesn't exist
        if !config_dir.exists() {
            std::fs::create_dir_all(&config_dir).map_err(|e| ConfigError::Foreign(Box::new(e)))?;
        }

        let mut builder = Config::builder()
            // Start with default values
            .add_source(Config::try_from(&Self::default())?);

        // Add configuration from file (optional)
        if let Some(path_str) = config_file.to_str() {
            builder = builder.add_source(File::with_name(path_str).required(false));
        } else {
            tracing::warn!("config file path contains invalid UTF-8 characters");
        }

        let config = builder.build()?;
        let mut settings: Self = config.try_deserialize()?;

        // Validate and reset hotkey if invalid
        settings.validate_hotkey();

        Ok(settings)
    }

    /// Save settings to configuration file
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_file = Self::config_file()?;
        let toml_string =
            toml::to_string_pretty(self).map_err(|e| ConfigError::Foreign(Box::new(e)))?;

        std::fs::write(&config_file, toml_string).map_err(|e| ConfigError::Foreign(Box::new(e)))?;
        Ok(())
    }

    /// Validate hotkey and reset to default if invalid
    fn validate_hotkey(&mut self) {
        if self.hotkey.activation_key.is_empty()
            || global_hotkey::hotkey::HotKey::from_str(&self.hotkey.activation_key).is_err()
        {
            self.hotkey.activation_key = Self::default().hotkey.activation_key;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub enum AppTheme {
    Light,
    Dark,
    #[default]
    System,
}

impl AppTheme {
    pub fn get_theme(&self) -> Self {
        match self {
            Self::System => match dark_light::detect().unwrap_or(dark_light::Mode::Light) {
                dark_light::Mode::Dark => Self::Dark,
                _ => Self::Light,
            },
            _ => self.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmMode {
    #[default]
    CopyToClipboard,
    PasteImmediately,
}

impl ConfirmMode {
    pub const fn requires_clipboard_completion(self) -> bool {
        matches!(self, Self::PasteImmediately)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfirmSettings {
    /// Behavior when confirming a record from the board
    pub mode: ConfirmMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeySettings {
    /// Global hotkey to activate clipboard manager (e.g., "`cmd+shift+v`")
    pub activation_key: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            #[cfg(target_os = "macos")]
            activation_key: "control+shift+d".to_string(),
            #[cfg(target_os = "windows")]
            activation_key: "ctrl+shift+d".to_string(),
            #[cfg(target_os = "linux")]
            activation_key: "ctrl+shift+d".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSettings {
    /// Maximum number of records to display in the UI (1 - 10,000)
    pub max_history_records: usize,
    /// Maximum number of records to store in the repository (1 - 100,000)
    /// Must be >= `max_history_records`
    pub max_storage_records: usize,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            max_history_records: DEFAULT_MAX_HISTORY_RECORDS,
            max_storage_records: DEFAULT_MAX_STORAGE_RECORDS,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AutoStartSettings {
    /// Whether to enable auto-launch at system startup
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettings {
    /// Whether to automatically check for updates on startup
    pub auto_check: bool,
    /// Check interval in hours
    pub check_interval_hours: u64,
    /// Whether to include pre-release versions
    pub include_prerelease: bool,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            auto_check: true,
            check_interval_hours: DEFAULT_UPDATE_CHECK_INTERVAL_HOURS,
            include_prerelease: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreviewSettings {
    /// Whether to enable hover preview for clipboard items
    pub hover_preview_enabled: bool,
}

impl Default for PreviewSettings {
    fn default() -> Self {
        Self {
            hover_preview_enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(
            settings.storage.max_history_records,
            DEFAULT_MAX_HISTORY_RECORDS
        );
        assert_eq!(
            settings.storage.max_storage_records,
            DEFAULT_MAX_STORAGE_RECORDS
        );
        assert_eq!(settings.confirm.mode, ConfirmMode::CopyToClipboard);
    }

    #[test]
    fn test_load_settings() {
        // This should work with default values even if no config file exists
        let result = Settings::load();
        assert!(result.is_ok());
    }

    #[test]
    fn test_app_theme() {
        let light = AppTheme::Light;
        assert!(matches!(light.get_theme(), AppTheme::Light));

        let dark = AppTheme::Dark;
        assert!(matches!(dark.get_theme(), AppTheme::Dark));

        // System theme should return either Light or Dark
        let system = AppTheme::System;
        let resolved = system.get_theme();
        assert!(matches!(resolved, AppTheme::Light | AppTheme::Dark));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_confirm_mode_serialization() {
        let toml = toml::to_string(&ConfirmSettings {
            mode: ConfirmMode::PasteImmediately,
        })
        .unwrap();
        assert!(toml.contains("paste_immediately"));

        let parsed: ConfirmSettings = toml::from_str(&toml).unwrap();
        assert_eq!(parsed.mode, ConfirmMode::PasteImmediately);
    }
}

use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Application settings structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// Hotkey configuration
    pub hotkey: HotkeySettings,
    /// Storage configuration
    pub storage: StorageSettings,
    /// Theme configuration
    pub theme: AppTheme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AppTheme {
    Light,
    Dark,
    System,
}

impl AppTheme {
    pub fn get_theme(&self) -> Self {
        match self {
            AppTheme::System => match dark_light::detect().unwrap() {
                dark_light::Mode::Dark => AppTheme::Dark,
                dark_light::Mode::Light => AppTheme::Light,
                _ => AppTheme::Light,
            },
            _ => self.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeySettings {
    /// Global hotkey to activate clipboard manager (e.g., "cmd+shift+v")
    pub activation_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSettings {
    /// Maximum number of records to keep in history
    pub max_history_records: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: HotkeySettings {
                #[cfg(target_os = "macos")]
                activation_key: "control+shift+d".to_string(),
                #[cfg(target_os = "windows")]
                activation_key: "ctrl+shift+d".to_string(),
            },
            storage: StorageSettings {
                max_history_records: 100,
            },
            theme: AppTheme::System,
        }
    }
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

        let builder = Config::builder()
            // Start with default values
            .add_source(Config::try_from(&Settings::default())?)
            // Add configuration from file (optional)
            .add_source(File::with_name(config_file.to_str().unwrap()).required(false));

        let config = builder.build()?;
        config.try_deserialize()
    }

    /// Save settings to configuration file
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_file = Self::config_file()?;
        let toml_string =
            toml::to_string_pretty(self).map_err(|e| ConfigError::Foreign(Box::new(e)))?;

        std::fs::write(&config_file, toml_string).map_err(|e| ConfigError::Foreign(Box::new(e)))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = Settings::default();
        assert_eq!(settings.storage.max_history_records, 100);
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
}

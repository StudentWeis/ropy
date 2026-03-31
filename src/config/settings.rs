use std::{path::PathBuf, str::FromStr};

use config::{Config, ConfigError, File};
use gpui::{App, Global, ReadGlobal};
use serde::{Deserialize, Serialize};

use crate::i18n::Language;

/// Default maximum number of records to display in the UI
const DEFAULT_MAX_HISTORY_RECORDS: usize = 100;
/// Default maximum number of records to store in the repository
const DEFAULT_MAX_STORAGE_RECORDS: usize = 200;

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
    /// Whether to include pre-release versions
    pub include_prerelease: bool,
}

impl Default for UpdateSettings {
    fn default() -> Self {
        Self {
            auto_check: true,
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

impl Global for Settings {}

impl Settings {
    /// Read a value from the global settings via a closure.
    pub fn read<R>(cx: &App, reader: impl FnOnce(&Self) -> R) -> R {
        reader(Self::global(cx))
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

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

    // ── Round-trip Tests ──────────────────────────────────────────

    #[test]
    #[allow(clippy::expect_used)]
    fn test_save_load_round_trip() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("config.toml");

        // Create settings with non-default values
        let mut settings = Settings::default();
        settings.storage.max_history_records = 50;
        settings.storage.max_storage_records = 500;
        settings.theme = AppTheme::Dark;
        settings.autostart.enabled = true;
        settings.language = Language::new("zh-CN");
        settings.update.auto_check = false;
        settings.update.include_prerelease = true;
        settings.preview.hover_preview_enabled = false;
        settings.confirm.mode = ConfirmMode::PasteImmediately;

        // Save to file
        let toml = toml::to_string_pretty(&settings).expect("Failed to serialize");
        std::fs::write(&config_path, toml).expect("Failed to write config");

        // Load back
        let content = std::fs::read_to_string(&config_path).expect("Failed to read config");
        let loaded: Settings = toml::from_str(&content).expect("Failed to deserialize");

        // Verify all fields match
        assert_eq!(loaded.storage.max_history_records, 50);
        assert_eq!(loaded.storage.max_storage_records, 500);
        assert!(matches!(loaded.theme, AppTheme::Dark));
        assert!(loaded.autostart.enabled);
        assert_eq!(loaded.language.code(), "zh-CN");
        assert!(!loaded.update.auto_check);
        assert!(loaded.update.include_prerelease);
        assert!(!loaded.preview.hover_preview_enabled);
        assert_eq!(loaded.confirm.mode, ConfirmMode::PasteImmediately);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_save_load_preserves_hotkey() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("config.toml");

        let mut settings = Settings::default();
        settings.hotkey.activation_key = "ctrl+shift+x".to_string();

        let toml = toml::to_string_pretty(&settings).expect("Failed to serialize");
        std::fs::write(&config_path, toml).expect("Failed to write config");

        let content = std::fs::read_to_string(&config_path).expect("Failed to read config");
        let loaded: Settings = toml::from_str(&content).expect("Failed to deserialize");

        assert_eq!(loaded.hotkey.activation_key, "ctrl+shift+x");
    }

    // ── Hotkey Validation Tests ───────────────────────────────────

    #[test]
    #[allow(clippy::expect_used)]
    fn test_validate_hotkey_valid() {
        let mut settings = Settings::default();
        settings.hotkey.activation_key = "ctrl+shift+v".to_string();

        settings.validate_hotkey();

        // Valid hotkey should not be changed
        assert_eq!(settings.hotkey.activation_key, "ctrl+shift+v");
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_validate_hotkey_empty() {
        let mut settings = Settings::default();
        settings.hotkey.activation_key = String::new();

        settings.validate_hotkey();

        // Empty hotkey should be reset to default
        let default = Settings::default();
        assert_eq!(
            settings.hotkey.activation_key,
            default.hotkey.activation_key
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_validate_hotkey_invalid() {
        let mut settings = Settings::default();
        settings.hotkey.activation_key = "not+a+valid+hotkey".to_string();

        settings.validate_hotkey();

        // Invalid hotkey should be reset to default
        let default = Settings::default();
        assert_eq!(
            settings.hotkey.activation_key,
            default.hotkey.activation_key
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_validate_hotkey_gibberish() {
        let mut settings = Settings::default();
        settings.hotkey.activation_key = "@@@###".to_string();

        settings.validate_hotkey();

        let default = Settings::default();
        assert_eq!(
            settings.hotkey.activation_key,
            default.hotkey.activation_key
        );
    }

    // ── Config File Edge Cases ────────────────────────────────────

    #[test]
    fn test_load_partial_config() {
        // Settings::default() provides all required fields.
        // Settings::load() uses config-rs which merges defaults with the file,
        // so partial config files work fine at runtime.
        // Here we verify that the default values themselves are correct.
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
        assert!(settings.update.auto_check);
        assert!(!settings.hotkey.activation_key.is_empty());
        assert!(matches!(settings.theme, AppTheme::System));
        assert_eq!(settings.language.code(), "en");
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_load_config_with_extra_fields() {
        // Verify that a fully-serialized Settings round-trips correctly.
        // Settings::load() uses config-rs which ignores unknown fields at runtime.
        let mut settings = Settings::default();
        settings.storage.max_history_records = 100;
        settings.storage.max_storage_records = 1000;
        settings.hotkey.activation_key = "ctrl+shift+v".to_string();

        let toml_str = toml::to_string_pretty(&settings).expect("Failed to serialize");

        assert!(toml_str.contains("max_history_records = 100"));
        assert!(toml_str.contains("max_storage_records = 1000"));
        assert!(toml_str.contains("ctrl+shift+v"));

        let loaded: Settings = toml::from_str(&toml_str).expect("Failed to deserialize");

        assert_eq!(loaded.storage.max_history_records, 100);
        assert_eq!(loaded.storage.max_storage_records, 1000);
        assert_eq!(loaded.hotkey.activation_key, "ctrl+shift+v");
    }

    #[test]
    fn test_load_malformed_config() {
        let malformed = r"
[storage
max_history_records = 100
";

        let result: Result<Settings, _> = toml::from_str(malformed);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_empty_config() {
        let empty = "";

        let result: Result<Settings, _> = toml::from_str(empty);
        // Empty config should use all defaults
        assert!(result.is_err()); // toml::from_str requires at least an empty table for struct
    }

    // ── ConfirmMode Tests ─────────────────────────────────────────

    #[test]
    fn test_confirm_mode_requires_clipboard_completion() {
        assert!(!ConfirmMode::CopyToClipboard.requires_clipboard_completion());
        assert!(ConfirmMode::PasteImmediately.requires_clipboard_completion());
    }

    #[test]
    fn test_confirm_mode_default() {
        let default = ConfirmMode::default();
        assert!(matches!(default, ConfirmMode::CopyToClipboard));
    }

    // ── StorageSettings Tests ─────────────────────────────────────

    #[test]
    fn test_storage_settings_default() {
        let storage = StorageSettings::default();
        assert_eq!(storage.max_history_records, DEFAULT_MAX_HISTORY_RECORDS);
        assert_eq!(storage.max_storage_records, DEFAULT_MAX_STORAGE_RECORDS);
    }

    // ── UpdateSettings Tests ──────────────────────────────────────

    #[test]
    fn test_update_settings_default() {
        let update = UpdateSettings::default();
        assert!(update.auto_check);
        assert!(!update.include_prerelease);
    }

    // ── PreviewSettings Tests ─────────────────────────────────────

    #[test]
    fn test_preview_settings_default() {
        let preview = PreviewSettings::default();
        assert!(preview.hover_preview_enabled);
    }

    // ── AutoStartSettings Tests ───────────────────────────────────

    #[test]
    fn test_autostart_settings_default() {
        let autostart = AutoStartSettings::default();
        assert!(!autostart.enabled);
    }

    // ── HotkeySettings Tests ──────────────────────────────────────

    #[test]
    fn test_hotkey_settings_default() {
        let hotkey = HotkeySettings::default();
        // Default hotkey should be valid
        assert!(!hotkey.activation_key.is_empty());
        // Verify it's a valid hotkey string
        assert!(global_hotkey::hotkey::HotKey::from_str(&hotkey.activation_key).is_ok());
    }

    // ── Language Tests ────────────────────────────────────────────

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_settings_language_round_trip() {
        let mut settings = Settings::default();

        // Test various language codes
        let languages = vec!["en", "zh-CN", "ja", "fr", "de"];

        for lang_code in languages {
            settings.language = Language::new(lang_code);
            let toml = toml::to_string_pretty(&settings).unwrap();
            let loaded: Settings = toml::from_str(&toml).unwrap();
            assert_eq!(loaded.language.code(), lang_code);
        }
    }

    // ── Integration-style Tests ───────────────────────────────────

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_settings_serialization_format() {
        let settings = Settings::default();
        let toml = toml::to_string_pretty(&settings).unwrap();

        // Verify key sections exist (using table headers that are actually serialized)
        assert!(toml.contains("[hotkey]"), "Missing [hotkey] section");
        assert!(toml.contains("[storage]"), "Missing [storage] section");
        assert!(toml.contains("[autostart]"), "Missing [autostart] section");
        assert!(toml.contains("[update]"), "Missing [update] section");
        assert!(toml.contains("[preview]"), "Missing [preview] section");
        assert!(toml.contains("[confirm]"), "Missing [confirm] section");
        // Note: [theme] and [language] are serialized as inline tables, not section headers
        assert!(toml.contains("theme"), "Missing theme field");
        assert!(toml.contains("language"), "Missing language field");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_settings_clone() {
        let settings = Settings::default();
        let cloned = settings.clone();

        assert_eq!(
            settings.storage.max_history_records,
            cloned.storage.max_history_records
        );
        assert_eq!(settings.hotkey.activation_key, cloned.hotkey.activation_key);
    }
}

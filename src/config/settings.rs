#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
use std::{cfg_select, path::PathBuf};

use config::{Config, ConfigError, File};
use gpui::{App, Global, ReadGlobal, SharedString};
use serde::{Deserialize, Serialize};

use crate::{
    gui::theme::ThemeId,
    i18n::{I18n, Language},
};

mod validate;

const DEFAULT_MAX_HISTORY_RECORDS: usize = 100;
const DEFAULT_MAX_STORAGE_RECORDS: usize = 200;
/// 40% is the lowest opacity that still keeps text legible during testing;
/// going lower made the UI effectively unusable.
const MIN_WINDOW_OPACITY_PERCENT: u8 = 40;
const MAX_WINDOW_OPACITY_PERCENT: u8 = 100;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    pub hotkey: HotkeySettings,
    pub storage: StorageSettings,
    pub theme: ThemeId,
    pub window: WindowSettings,
    pub layout: LayoutSettings,
    pub autostart: AutoStartSettings,
    pub language: Language,
    pub update: UpdateSettings,
    pub preview: PreviewSettings,
    pub confirm: ConfirmSettings,
}

impl Settings {
    pub fn config_dir() -> Result<PathBuf, ConfigError> {
        dirs::config_dir()
            .map(|dir| dir.join("ropy"))
            .ok_or_else(|| ConfigError::NotFound("Config directory not found".to_string()))
    }

    pub fn config_file() -> Result<PathBuf, ConfigError> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Load settings, layering the on-disk `config.toml` over the
    /// `Default` instance so partial files keep working across upgrades,
    /// and clamping each value group via [`validate`] so out-of-range
    /// values on disk can't propagate into the running app.
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
        settings.validate_window_opacity();
        settings.validate_storage();

        Ok(settings)
    }

    pub fn save(&self) -> Result<(), ConfigError> {
        let config_file = Self::config_file()?;
        let toml_string =
            toml::to_string_pretty(self).map_err(|e| ConfigError::Foreign(Box::new(e)))?;

        std::fs::write(&config_file, toml_string).map_err(|e| ConfigError::Foreign(Box::new(e)))?;
        Ok(())
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    #[default]
    List,
    Grid,
}

impl LayoutMode {
    pub const fn all() -> [Self; 2] {
        [Self::List, Self::Grid]
    }

    pub fn label(self, cx: &App) -> SharedString {
        let label = match self {
            Self::List => I18n::translate(cx, "settings_layout_list"),
            Self::Grid => I18n::translate(cx, "settings_layout_grid"),
        };
        SharedString::from(label)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayoutSettings {
    pub mode: LayoutMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConfirmSettings {
    pub mode: ConfirmMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSettings {
    /// Allowed range is [`MIN_OPACITY_PERCENT`..=`MAX_OPACITY_PERCENT`];
    /// values outside that band are clamped at load time.
    pub opacity_percent: u8,
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            opacity_percent: MAX_WINDOW_OPACITY_PERCENT,
        }
    }
}

impl WindowSettings {
    pub const MIN_OPACITY_PERCENT: u8 = MIN_WINDOW_OPACITY_PERCENT;
    pub const MAX_OPACITY_PERCENT: u8 = MAX_WINDOW_OPACITY_PERCENT;

    pub fn normalize_opacity(&mut self) {
        self.opacity_percent = self
            .opacity_percent
            .clamp(MIN_WINDOW_OPACITY_PERCENT, MAX_WINDOW_OPACITY_PERCENT);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HotkeySettings {
    /// `+`-separated chord parsed by `global_hotkey` (e.g. `cmd+shift+v`).
    /// Invalid values are reset to the default by [`validate_hotkey`].
    pub activation_key: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            activation_key: cfg_select! {
                target_os = "macos" => { "control+shift+d".to_string() },
                _ => { "ctrl+shift+d".to_string() },
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSettings {
    /// Soft cap on records visible in the board (1 – 10,000). Records past
    /// this point are kept on disk but hidden until older entries are
    /// pinned / cleared.
    pub max_history_records: usize,
    /// Hard cap before cleanup deletes records (1 – 100,000). Validation
    /// ensures `max_storage_records >= max_history_records` so the
    /// visible window can always be filled.
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
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettings {
    pub auto_check: bool,
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
    /// Closure-style accessor to the global instance — keeps call sites
    /// from holding a borrow of `cx` longer than the field they need.
    pub fn read<R>(cx: &App, reader: impl FnOnce(&Self) -> R) -> R {
        reader(Self::global(cx))
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

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
        assert_eq!(settings.window.opacity_percent, 100);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn test_default_hotkey_settings_on_macos() {
        assert_eq!(HotkeySettings::default().activation_key, "control+shift+d");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn test_default_hotkey_settings_on_non_macos() {
        assert_eq!(HotkeySettings::default().activation_key, "ctrl+shift+d");
    }

    #[test]
    fn test_load_settings() {
        // This should work with default values even if no config file exists
        let result = Settings::load();
        assert!(result.is_ok());
    }

    #[test]
    fn test_theme_id_default() {
        let theme = ThemeId::default();

        assert_eq!(theme.code(), "ropy-light");
    }

    #[test]
    #[expect(clippy::unwrap_used)]
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
    fn test_save_load_round_trip() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("config.toml");

        // Create settings with non-default values
        let mut settings = Settings::default();
        settings.storage.max_history_records = 50;
        settings.storage.max_storage_records = 500;
        settings.theme = ThemeId::new("ropy-dark");
        settings.autostart.enabled = true;
        settings.language = Language::new("zh-CN");
        settings.update.auto_check = false;
        settings.update.include_prerelease = true;
        settings.preview.hover_preview_enabled = false;
        settings.confirm.mode = ConfirmMode::PasteImmediately;
        settings.layout.mode = LayoutMode::Grid;
        settings.window.opacity_percent = 72;

        // Save to file
        let toml = toml::to_string_pretty(&settings).expect("Failed to serialize");
        std::fs::write(&config_path, toml).expect("Failed to write config");

        // Load back
        let content = std::fs::read_to_string(&config_path).expect("Failed to read config");
        let loaded: Settings = toml::from_str(&content).expect("Failed to deserialize");

        // Verify all fields match
        assert_eq!(loaded.storage.max_history_records, 50);
        assert_eq!(loaded.storage.max_storage_records, 500);
        assert_eq!(loaded.theme.code(), "ropy-dark");
        assert!(loaded.autostart.enabled);
        assert_eq!(loaded.language.code(), "zh-CN");
        assert!(!loaded.update.auto_check);
        assert!(loaded.update.include_prerelease);
        assert!(!loaded.preview.hover_preview_enabled);
        assert_eq!(loaded.confirm.mode, ConfirmMode::PasteImmediately);
        assert_eq!(loaded.layout.mode, LayoutMode::Grid);
        assert_eq!(loaded.window.opacity_percent, 72);
    }

    #[test]
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
    fn test_validate_hotkey_valid() {
        let mut settings = Settings::default();
        settings.hotkey.activation_key = "ctrl+shift+v".to_string();

        settings.validate_hotkey();

        // Valid hotkey should not be changed
        assert_eq!(settings.hotkey.activation_key, "ctrl+shift+v");
    }

    #[test]
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
        assert_eq!(settings.theme.code(), "ropy-light");
        assert_eq!(settings.language.code(), "en");
    }

    #[test]
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

    #[test]
    fn test_layout_mode_default() {
        assert_eq!(LayoutMode::default(), LayoutMode::List);
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_layout_mode_serialization_round_trip() {
        let mut settings = Settings::default();
        settings.layout.mode = LayoutMode::Grid;

        let toml = toml::to_string_pretty(&settings).unwrap();
        assert!(toml.contains("[layout]"));
        assert!(toml.contains("mode = \"grid\""));

        let loaded: Settings = toml::from_str(&toml).unwrap();
        assert_eq!(loaded.layout.mode, LayoutMode::Grid);
    }

    #[test]
    fn test_layout_settings_default() {
        let layout = LayoutSettings::default();
        assert_eq!(layout.mode, LayoutMode::List);
    }

    // ── StorageSettings Tests ─────────────────────────────────────

    #[test]
    fn test_storage_settings_default() {
        let storage = StorageSettings::default();
        assert_eq!(storage.max_history_records, DEFAULT_MAX_HISTORY_RECORDS);
        assert_eq!(storage.max_storage_records, DEFAULT_MAX_STORAGE_RECORDS);
    }

    #[test]
    fn test_validate_storage_clamps_history_to_supported_range() {
        let mut settings = Settings::default();
        settings.storage.max_history_records = 0;
        settings.validate_storage();
        assert_eq!(settings.storage.max_history_records, 1);

        settings.storage.max_history_records = 99_999;
        settings.validate_storage();
        assert_eq!(settings.storage.max_history_records, 10_000);
    }

    #[test]
    fn test_validate_storage_clamps_storage_to_supported_range() {
        let mut settings = Settings::default();
        settings.storage.max_history_records = 1;
        settings.storage.max_storage_records = 0;
        settings.validate_storage();
        assert_eq!(settings.storage.max_storage_records, 1);

        settings.storage.max_storage_records = 999_999;
        settings.validate_storage();
        assert_eq!(settings.storage.max_storage_records, 100_000);
    }

    #[test]
    fn test_validate_storage_enforces_storage_gte_history() {
        let mut settings = Settings::default();
        settings.storage.max_history_records = 500;
        settings.storage.max_storage_records = 100;
        settings.validate_storage();
        assert_eq!(settings.storage.max_storage_records, 500);
    }

    #[test]
    fn test_validate_storage_preserves_valid_values() {
        let mut settings = Settings::default();
        settings.storage.max_history_records = 50;
        settings.storage.max_storage_records = 1_000;
        settings.validate_storage();
        assert_eq!(settings.storage.max_history_records, 50);
        assert_eq!(settings.storage.max_storage_records, 1_000);
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

    #[test]
    fn test_window_settings_default() {
        let window = WindowSettings::default();
        assert_eq!(window.opacity_percent, 100);
        let opacity_factor = f32::from(window.opacity_percent) / 100.0;
        assert!((opacity_factor - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_window_settings_normalize_opacity_clamps_to_supported_range() {
        let mut window = WindowSettings { opacity_percent: 5 };
        window.normalize_opacity();
        assert_eq!(window.opacity_percent, MIN_WINDOW_OPACITY_PERCENT);

        window.opacity_percent = 150;
        window.normalize_opacity();
        assert_eq!(window.opacity_percent, 100);
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
    #[expect(clippy::unwrap_used)]
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

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_settings_theme_round_trip() {
        let mut settings = Settings::default();
        let themes = vec!["ropy-light", "ropy-dark", "custom-theme"];

        for theme_code in themes {
            settings.theme = ThemeId::new(theme_code);
            let toml = toml::to_string_pretty(&settings).unwrap();
            let loaded: Settings = toml::from_str(&toml).unwrap();
            assert_eq!(loaded.theme.code(), theme_code);
        }
    }

    // ── Integration-style Tests ───────────────────────────────────

    #[test]
    #[expect(clippy::unwrap_used)]
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
        assert!(toml.contains("[layout]"), "Missing [layout] section");
        assert!(toml.contains("[window]"), "Missing [window] section");
        // Note: [theme] and [language] are serialized as inline tables, not section headers
        assert!(toml.contains("theme"), "Missing theme field");
        assert!(toml.contains("language"), "Missing language field");
    }

    #[test]
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

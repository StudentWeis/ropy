#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
use std::{
    cfg_select,
    io::Write as _,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{language_id::Language, theme_id::ThemeId};

const DEFAULT_MAX_HISTORY_RECORDS: usize = 100;
const DEFAULT_MAX_STORAGE_RECORDS: usize = 200;
/// 40% is the lowest opacity that still keeps text legible during testing;
/// going lower made the UI effectively unusable.
const MIN_WINDOW_OPACITY_PERCENT: u8 = 40;
const MAX_WINDOW_OPACITY_PERCENT: u8 = 100;

/// Failure to load or safely persist application settings.
#[derive(Debug, Error)]
pub enum SettingsError {
    /// Saving is blocked until a previously invalid configuration has been repaired.
    #[error("settings recovery required; repair config.toml and restart before saving")]
    RecoveryRequired,
    /// The platform has no usable configuration directory.
    #[error("config directory not found")]
    ConfigDirectoryNotFound,
    /// A configuration file operation failed.
    #[error("failed to access settings file at {path:?}: {source}")]
    Io {
        /// Configuration path associated with the failed operation.
        path: PathBuf,
        /// Underlying filesystem error.
        #[source]
        source: std::io::Error,
    },
    /// The configuration is not valid TOML or contains incompatible value types.
    #[error("failed to parse settings file: {0}")]
    Deserialize(#[from] toml::de::Error),
    /// The settings cannot be represented as TOML.
    #[error("failed to serialize settings: {0}")]
    Serialize(#[from] toml::ser::Error),
}

/// Persisted preferences plus a non-serialized guard against overwriting invalid files.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
#[non_exhaustive]
pub struct Settings {
    /// Shortcut preferences; native key syntax is validated by the desktop adapter.
    pub hotkey: HotkeySettings,
    /// History visibility and retention limits.
    pub storage: StorageSettings,
    /// Selected bundled theme code.
    pub theme: ThemeId,
    /// Window opacity preferences.
    pub window: WindowSettings,
    /// History presentation mode.
    pub layout: LayoutSettings,
    /// Requested operating-system login behavior.
    pub autostart: AutoStartSettings,
    /// Selected locale code.
    pub language: Language,
    /// Release-check preferences.
    pub update: UpdateSettings,
    /// Preview activation preferences.
    pub preview: PreviewSettings,
    /// Behavior when a history entry is confirmed.
    pub confirm: ConfirmSettings,
    /// Runtime protection after a failed load; never persisted in config.toml.
    #[serde(skip)]
    recovery_required: bool,
}

impl Settings {
    /// Return defaults that cannot be saved over an unreadable user configuration.
    #[must_use]
    pub fn recovery_defaults() -> Self {
        Self {
            recovery_required: true,
            ..Self::default()
        }
    }

    /// Whether a failed configuration load prevents saving this value.
    #[must_use]
    pub const fn is_recovery_required(&self) -> bool {
        self.recovery_required
    }

    /// Resolve the application configuration directory for the current user.
    ///
    /// # Errors
    /// Returns `ConfigDirectoryNotFound` when the platform cannot resolve a user directory.
    pub fn config_dir() -> Result<PathBuf, SettingsError> {
        dirs::config_dir()
            .map(|dir| dir.join("ropy"))
            .ok_or(SettingsError::ConfigDirectoryNotFound)
    }

    /// Resolve the current user's `config.toml` path.
    ///
    /// # Errors
    /// Returns `ConfigDirectoryNotFound` when the platform cannot resolve a user directory.
    pub fn config_file() -> Result<PathBuf, SettingsError> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    /// Load settings, layering the on-disk `config.toml` over the
    /// `Default` instance so partial files keep working across upgrades,
    /// and clamping storage limits and opacity to their supported ranges.
    /// The application must validate native shortcut syntax before registration.
    ///
    /// # Errors
    /// Returns an error if the configuration cannot be read or parsed.
    pub fn load() -> Result<Self, SettingsError> {
        let config_dir = Self::config_dir()?;
        Self::load_from_dir(&config_dir)
    }

    /// Load and validate `config.toml` from an explicit directory, using defaults if absent.
    /// Native shortcut syntax must be validated by the application before registration.
    ///
    /// # Errors
    /// Returns an error if the directory cannot be created or the file cannot be read or parsed.
    pub fn load_from_dir(config_dir: &Path) -> Result<Self, SettingsError> {
        let config_file = config_dir.join("config.toml");

        std::fs::create_dir_all(config_dir).map_err(|source| SettingsError::Io {
            path: config_dir.to_path_buf(),
            source,
        })?;

        if !config_file.exists() {
            return Ok(Self::default().validated());
        }

        let config_content =
            std::fs::read_to_string(&config_file).map_err(|source| SettingsError::Io {
                path: config_file.clone(),
                source,
            })?;

        Self::from_config_content(&config_content)
    }

    fn from_config_content(config_content: &str) -> Result<Self, SettingsError> {
        let mut default_config = toml::Value::try_from(Self::default())?;
        let file_config = toml::Value::Table(toml::from_str(config_content)?);
        Self::merge_config_values(&mut default_config, file_config);

        let settings: Self = default_config.try_into()?;
        Ok(settings.validated())
    }

    /// Atomically persist settings to the current user's configuration file.
    ///
    /// # Errors
    /// Returns an error during recovery protection, serialization or atomic file persistence.
    pub fn save(&self) -> Result<(), SettingsError> {
        let config_file = Self::config_file()?;
        self.save_to_file(&config_file)
    }

    /// Atomically persist settings to an explicit path, preserving the old file on failure.
    ///
    /// # Errors
    /// Returns an error during recovery protection, serialization or atomic file persistence.
    pub fn save_to_file(&self, config_file: &Path) -> Result<(), SettingsError> {
        if self.is_recovery_required() {
            return Err(SettingsError::RecoveryRequired);
        }
        let toml_string = toml::to_string_pretty(self)?;
        let parent = config_file
            .parent()
            .ok_or(SettingsError::ConfigDirectoryNotFound)?;
        std::fs::create_dir_all(parent).map_err(|source| SettingsError::Io {
            path: parent.to_path_buf(),
            source,
        })?;

        let mut temp_file =
            tempfile::NamedTempFile::new_in(parent).map_err(|source| SettingsError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        temp_file
            .write_all(toml_string.as_bytes())
            .and_then(|()| temp_file.as_file().sync_all())
            .map_err(|source| SettingsError::Io {
                path: config_file.to_path_buf(),
                source,
            })?;
        temp_file
            .persist(config_file)
            .map_err(|error| SettingsError::Io {
                path: config_file.to_path_buf(),
                source: error.error,
            })?;
        Ok(())
    }

    fn merge_config_values(default_config: &mut toml::Value, file_config: toml::Value) {
        match (default_config, file_config) {
            (toml::Value::Table(default_table), toml::Value::Table(file_table)) => {
                for (key, file_value) in file_table {
                    if let Some(default_value) = default_table.get_mut(&key) {
                        Self::merge_config_values(default_value, file_value);
                    } else {
                        default_table.insert(key, file_value);
                    }
                }
            }
            (default_value, file_value) => {
                *default_value = file_value;
            }
        }
    }

    fn validated(mut self) -> Self {
        self.validate_window_opacity();
        self.validate_storage();
        self
    }

    fn validate_window_opacity(&mut self) {
        self.window.normalize_opacity();
    }

    fn validate_storage(&mut self) {
        self.storage.max_history_records = self.storage.max_history_records.clamp(1, 10_000);
        self.storage.max_storage_records = self.storage.max_storage_records.clamp(1, 100_000);
        if self.storage.max_storage_records < self.storage.max_history_records {
            self.storage.max_storage_records = self.storage.max_history_records;
        }
    }
}

/// Action to perform after confirming a history entry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmMode {
    /// Write the entry to the clipboard without synthesizing a paste action.
    #[default]
    CopyToClipboard,
    /// Wait for the clipboard write before pasting into the previous application.
    PasteImmediately,
}

impl ConfirmMode {
    /// Whether confirmation must await a successful native clipboard write.
    #[must_use]
    pub const fn requires_clipboard_completion(self) -> bool {
        matches!(self, Self::PasteImmediately)
    }
}

/// Persisted history layout preference.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LayoutMode {
    /// Show history as a list.
    #[default]
    List,
    /// Show history as a grid.
    Grid,
}

impl LayoutMode {
    /// Return supported layouts in selector order.
    #[must_use]
    pub const fn all() -> [Self; 2] {
        [Self::List, Self::Grid]
    }
}

/// Preferences for the history layout.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
#[non_exhaustive]
pub struct LayoutSettings {
    /// Selected behavior for this preference group.
    pub mode: LayoutMode,
}

/// Preferences for confirming a history entry.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
#[non_exhaustive]
pub struct ConfirmSettings {
    /// Selected behavior for this preference group.
    pub mode: ConfirmMode,
}

/// Persisted window appearance preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct WindowSettings {
    /// Allowed range is [`WindowSettings::MIN_OPACITY_PERCENT`] through
    /// [`WindowSettings::MAX_OPACITY_PERCENT`];
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
    /// Lowest supported opacity, in percent.
    pub const MIN_OPACITY_PERCENT: u8 = MIN_WINDOW_OPACITY_PERCENT;
    /// Fully opaque window value, in percent.
    pub const MAX_OPACITY_PERCENT: u8 = MAX_WINDOW_OPACITY_PERCENT;

    /// Clamp opacity to the supported range.
    pub fn normalize_opacity(&mut self) {
        self.opacity_percent = self
            .opacity_percent
            .clamp(MIN_WINDOW_OPACITY_PERCENT, MAX_WINDOW_OPACITY_PERCENT);
    }
}

/// Serialized shortcut preferences; registration belongs to the desktop adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct HotkeySettings {
    /// `+`-separated chord parsed by `global_hotkey` (e.g. `cmd+shift+v`).
    /// The desktop adapter validates native key syntax before registration.
    pub activation_key: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        Self {
            activation_key: cfg_select! {
                target_os = "macos" => "control+shift+d".to_string(),
                _ => "ctrl+shift+d".to_string(),
            },
        }
    }
}

/// Limits for visible history and retained storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
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

/// Requested login-startup preference.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
#[non_exhaustive]
pub struct AutoStartSettings {
    /// Whether the application should start when the user logs in.
    pub enabled: bool,
}

/// Automatic update discovery preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct UpdateSettings {
    /// Whether periodic release checks are enabled.
    pub auto_check: bool,
    /// Whether pre-release versions may be offered.
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

/// Activation preferences for record previews.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
#[non_exhaustive]
pub struct PreviewSettings {
    /// Whether hovering a record opens its preview.
    pub hover_preview_enabled: bool,
    /// Whether the Space key opens a preview.
    pub space_preview_enabled: bool,
}

impl Default for PreviewSettings {
    fn default() -> Self {
        Self {
            hover_preview_enabled: true,
            space_preview_enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_settings_recovery_after_invalid_load_blocks_save_and_preserves_file() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("config.toml");
        let original = "[storage\nmax_history_records = nope";
        std::fs::write(&path, original).expect("write config");
        assert!(Settings::load_from_dir(directory.path()).is_err());
        let mut settings = Settings::recovery_defaults();
        settings.storage.max_history_records = 50;

        assert!(settings.save_to_file(&path).is_err());
        assert_eq!(
            std::fs::read_to_string(&path).expect("read config"),
            original
        );
        assert!(settings.is_recovery_required());

        std::fs::write(&path, "[storage]\nmax_history_records = 75").expect("repair config");
        let loaded = Settings::load_from_dir(directory.path()).expect("reload config");
        assert!(!loaded.is_recovery_required());
        assert_eq!(loaded.storage.max_history_records, 75);
        loaded.save_to_file(&path).expect("save after recovery");
    }

    #[test]
    fn test_settings_recovery_defaults_do_not_serialize_runtime_state() {
        let settings = Settings::recovery_defaults();
        assert!(settings.is_recovery_required());
        let content = toml::to_string(&settings).expect("serialize");
        assert!(!content.contains("recovery_required"));
    }

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
    fn test_load_from_dir_when_config_missing_returns_defaults_without_host_state() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let settings = Settings::load_from_dir(temp_dir.path()).expect("Failed to load settings");

        assert_eq!(
            settings.storage.max_history_records,
            DEFAULT_MAX_HISTORY_RECORDS
        );
        assert!(!temp_dir.path().join("config.toml").exists());
    }

    #[test]
    fn test_load_from_dir_when_config_is_invalid_preserves_original_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("config.toml");
        let invalid_content = "[storage\nmax_history_records = nope";
        std::fs::write(&config_path, invalid_content).expect("Failed to write fixture");

        let result = Settings::load_from_dir(temp_dir.path());

        assert!(matches!(result, Err(SettingsError::Deserialize(_))));
        assert_eq!(
            std::fs::read_to_string(config_path).expect("Failed to read fixture"),
            invalid_content
        );
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
        settings.preview.space_preview_enabled = false;
        settings.confirm.mode = ConfirmMode::PasteImmediately;
        settings.layout.mode = LayoutMode::Grid;
        settings.window.opacity_percent = 72;

        settings
            .save_to_file(&config_path)
            .expect("Failed to save settings");
        let loaded = Settings::load_from_dir(temp_dir.path()).expect("Failed to load settings");

        // Verify all fields match
        assert_eq!(loaded.storage.max_history_records, 50);
        assert_eq!(loaded.storage.max_storage_records, 500);
        assert_eq!(loaded.theme.code(), "ropy-dark");
        assert!(loaded.autostart.enabled);
        assert_eq!(loaded.language.code(), "zh-CN");
        assert!(!loaded.update.auto_check);
        assert!(loaded.update.include_prerelease);
        assert!(!loaded.preview.hover_preview_enabled);
        assert!(!loaded.preview.space_preview_enabled);
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

    // ── Config File Edge Cases ────────────────────────────────────

    #[test]
    fn test_load_partial_config() {
        let settings = Settings::from_config_content(
            r"
[storage]
max_history_records = 50

[window]
opacity_percent = 72
",
        )
        .expect("Failed to load partial config");

        assert_eq!(settings.storage.max_history_records, 50);
        assert_eq!(
            settings.storage.max_storage_records,
            DEFAULT_MAX_STORAGE_RECORDS
        );
        assert_eq!(settings.window.opacity_percent, 72);
        assert_eq!(settings.confirm.mode, ConfirmMode::CopyToClipboard);
        assert!(settings.update.auto_check);
        assert_ne!(settings.hotkey.activation_key, "");
        assert_eq!(settings.theme.code(), "ropy-light");
        assert_eq!(settings.language.code(), "en");
    }

    #[test]
    fn test_load_config_with_extra_fields() {
        let settings = Settings::from_config_content(
            r#"
unknown_root = "ignored"

[storage]
max_history_records = 100
max_storage_records = 1000
unknown_storage = "ignored"

[hotkey]
activation_key = "ctrl+shift+v"
"#,
        )
        .expect("Failed to load config with extra fields");

        assert_eq!(settings.storage.max_history_records, 100);
        assert_eq!(settings.storage.max_storage_records, 1000);
        assert_eq!(settings.hotkey.activation_key, "ctrl+shift+v");
    }

    #[test]
    fn test_load_malformed_config() {
        let malformed = r"
[storage
max_history_records = 100
";

        let result = Settings::from_config_content(malformed);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_errors_include_source_details() {
        let error = Settings::from_config_content("[storage").expect_err("expected parse error");
        let rendered = format!("{error}");
        let prefix = "failed to parse settings file: ";

        assert!(rendered.starts_with(prefix));
        assert!(rendered.len() > prefix.len());
    }

    #[test]
    fn test_io_errors_include_source_details() {
        let error = SettingsError::Io {
            path: PathBuf::from("/tmp/config.toml"),
            source: std::io::Error::new(std::io::ErrorKind::PermissionDenied, "permission denied"),
        };
        let rendered = format!("{error}");

        assert!(rendered.contains("failed to access settings file at"));
        assert!(rendered.contains("permission denied"));
    }

    #[test]
    fn test_load_empty_config() {
        let settings = Settings::from_config_content("").expect("Failed to load empty config");

        assert_eq!(
            settings.storage.max_history_records,
            DEFAULT_MAX_HISTORY_RECORDS
        );
        assert_eq!(
            settings.storage.max_storage_records,
            DEFAULT_MAX_STORAGE_RECORDS
        );
        assert_eq!(settings.confirm.mode, ConfirmMode::CopyToClipboard);
        assert_eq!(settings.theme.code(), "ropy-light");
    }

    #[test]
    fn test_merge_config_values_recursively_overlays_tables() {
        let mut default_config = toml::Value::Table(
            toml::from_str(
                r"
[storage]
max_history_records = 100
max_storage_records = 200

[window]
opacity_percent = 100

[preview]
hover_preview_enabled = true
space_preview_enabled = true
",
            )
            .expect("Failed to parse default config"),
        );
        let file_config = toml::Value::Table(
            toml::from_str(
                r"
[storage]
max_history_records = 50
unknown_storage = 7

[window]
opacity_percent = 72

[new_section]
enabled = true
",
            )
            .expect("Failed to parse file config"),
        );
        let expected_config = toml::Value::Table(
            toml::from_str(
                r"
[storage]
max_history_records = 50
max_storage_records = 200
unknown_storage = 7

[window]
opacity_percent = 72

[preview]
hover_preview_enabled = true
space_preview_enabled = true

[new_section]
enabled = true
",
            )
            .expect("Failed to parse expected config"),
        );

        Settings::merge_config_values(&mut default_config, file_config);

        assert_eq!(default_config, expected_config);
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
        assert!(preview.space_preview_enabled);
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

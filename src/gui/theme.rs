use std::{collections::HashMap, sync::OnceLock};

use rust_embed::RustEmbed;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;

/// Bundled theme TOML files. Adding a `<id>.toml` under `assets/themes/`
/// is enough to make the theme selectable — no Rust code change required.
#[derive(RustEmbed)]
#[folder = "assets/themes"]
struct ThemeAssets;

static DISPLAY_NAMES: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Theme identifier — the bundle file name without the `.toml` suffix.
/// Serialized transparently as the raw string so existing `config.toml`
/// values keep working.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ThemeId(String);

impl ThemeId {
    pub fn new(code: impl Into<String>) -> Self {
        Self(normalize_theme_code(&code.into()))
    }

    pub fn code(&self) -> &str {
        &self.0
    }

    pub fn display_name(&self) -> String {
        cached_display_names()
            .get(self.code())
            .cloned()
            .unwrap_or_else(|| self.0.clone())
    }

    pub fn all() -> Vec<Self> {
        let mut codes: Vec<String> = ThemeAssets::iter()
            .filter_map(|name| name.as_ref().strip_suffix(".toml").map(str::to_owned))
            .collect();
        codes.sort();
        codes.into_iter().map(Self).collect()
    }
}

impl Default for ThemeId {
    fn default() -> Self {
        Self::new("ropy-light")
    }
}

impl<'de> Deserialize<'de> for ThemeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let raw = String::deserialize(deserializer)?;
        Ok(Self::new(raw))
    }
}

/// Appearance hint required by `gpui-component`'s theme runtime so it can
/// pick the right base styles before our palette is layered on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeDefinition {
    id: ThemeId,
    name: String,
    mode: ThemeMode,
    palette: ThemePalette,
}

impl ThemeDefinition {
    pub fn load(theme_id: &ThemeId) -> Result<Self, ThemeError> {
        let file_name = format!("{}.toml", theme_id.code());
        let file = ThemeAssets::get(&file_name)
            .ok_or_else(|| ThemeError::NotFound(theme_id.code().to_string()))?;
        let content =
            std::str::from_utf8(&file.data).map_err(|source| ThemeError::InvalidUtf8 {
                theme_id: theme_id.code().to_string(),
                source,
            })?;
        let raw = toml::from_str::<ThemeDefinitionRaw>(content).map_err(|source| {
            ThemeError::ParseToml {
                theme_id: theme_id.code().to_string(),
                source,
            }
        })?;

        Self::try_from_raw(theme_id.clone(), raw)
    }

    pub fn load_or_default(theme_id: &ThemeId) -> Self {
        Self::load(theme_id).unwrap_or_else(|error| {
            tracing::warn!(
                error = %error,
                requested_theme = theme_id.code(),
                fallback_theme = ThemeId::default().code(),
                "failed to load requested theme; falling back to default theme"
            );

            Self::load(&ThemeId::default()).unwrap_or_else(|fallback_error| {
                tracing::error!(
                    error = %fallback_error,
                    "failed to load fallback theme; using emergency built-in theme"
                );
                Self::fallback()
            })
        })
    }

    pub const fn mode(&self) -> ThemeMode {
        self.mode
    }

    pub const fn palette(&self) -> &ThemePalette {
        &self.palette
    }

    fn try_from_raw(id: ThemeId, raw: ThemeDefinitionRaw) -> Result<Self, ThemeError> {
        let palette = ThemePalette {
            background: parse_color(&id, "background", &raw.background)?,
            foreground: parse_color(&id, "foreground", &raw.foreground)?,
            secondary: parse_color(&id, "secondary", &raw.secondary)?,
            secondary_foreground: parse_color(
                &id,
                "secondary_foreground",
                &raw.secondary_foreground,
            )?,
            border: parse_color(&id, "border", &raw.border)?,
            accent: parse_color(&id, "accent", &raw.accent)?,
            accent_foreground: parse_color(&id, "accent_foreground", &raw.accent_foreground)?,
            muted: parse_color(&id, "muted", &raw.muted)?,
            muted_foreground: parse_color(&id, "muted_foreground", &raw.muted_foreground)?,
            input: parse_color(&id, "input", &raw.input)?,
            primary: parse_color(&id, "primary", &raw.primary)?,
            primary_foreground: parse_color(&id, "primary_foreground", &raw.primary_foreground)?,
            primary_hover: parse_color(&id, "primary_hover", &raw.primary_hover)?,
            primary_active: parse_color(&id, "primary_active", &raw.primary_active)?,
            danger: parse_color(&id, "danger", &raw.danger)?,
            danger_foreground: parse_color(&id, "danger_foreground", &raw.danger_foreground)?,
            popover: parse_color(&id, "popover", &raw.popover)?,
            popover_foreground: parse_color(&id, "popover_foreground", &raw.popover_foreground)?,
            selection: parse_color(&id, "selection", &raw.selection)?,
            ring: parse_color(&id, "ring", &raw.ring)?,
            list_hover: parse_color(&id, "list_hover", &raw.list_hover)?,
            list_active: parse_color(&id, "list_active", &raw.list_active)?,
            scrollbar_thumb: parse_color(&id, "scrollbar_thumb", &raw.scrollbar_thumb)?,
        };

        Ok(Self {
            id,
            name: raw.theme_name,
            mode: raw.mode,
            palette,
        })
    }

    fn fallback() -> Self {
        Self {
            id: ThemeId::default(),
            name: "Ropy Light".to_string(),
            mode: ThemeMode::Light,
            palette: ThemePalette {
                background: 0x00ff_ffff,
                foreground: 0x001a_1a1a,
                secondary: 0x00f5_f5f5,
                secondary_foreground: 0x001a_1a1a,
                border: 0x00e0_e0e0,
                accent: 0x00ad_d8e6,
                accent_foreground: 0x001a_1a1a,
                muted: 0x00f5_f5f5,
                muted_foreground: 0x006b_6b6b,
                input: 0x00f0_f0f0,
                primary: 0x005b_7bd5,
                primary_foreground: 0x00ff_ffff,
                primary_hover: 0x004e_70c2,
                primary_active: 0x0042_60ab,
                danger: 0x00d9_5c5c,
                danger_foreground: 0x00ff_ffff,
                popover: 0x00ff_ffff,
                popover_foreground: 0x001a_1a1a,
                selection: 0x00d9_e4f9,
                ring: 0x005b_7bd5,
                list_hover: 0x00f5_f5f5,
                list_active: 0x00e8_edf8,
                scrollbar_thumb: 0x00c8_d2e8,
            },
        }
    }
}

/// Palette values copied into `gpui-component::theme::Theme` at apply time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemePalette {
    pub background: u32,
    pub foreground: u32,
    pub secondary: u32,
    pub secondary_foreground: u32,
    pub border: u32,
    pub accent: u32,
    pub accent_foreground: u32,
    pub muted: u32,
    pub muted_foreground: u32,
    pub input: u32,
    pub primary: u32,
    pub primary_foreground: u32,
    pub primary_hover: u32,
    pub primary_active: u32,
    pub danger: u32,
    pub danger_foreground: u32,
    pub popover: u32,
    pub popover_foreground: u32,
    pub selection: u32,
    pub ring: u32,
    pub list_hover: u32,
    pub list_active: u32,
    pub scrollbar_thumb: u32,
}

#[derive(Debug, Error)]
pub enum ThemeError {
    #[error("theme '{0}' was not found")]
    NotFound(String),
    #[error("theme '{theme_id}' contains invalid UTF-8")]
    InvalidUtf8 {
        theme_id: String,
        #[source]
        source: std::str::Utf8Error,
    },
    #[error("theme '{theme_id}' could not be parsed as TOML")]
    ParseToml {
        theme_id: String,
        #[source]
        source: toml::de::Error,
    },
    #[error("theme '{theme_id}' has an invalid color for '{field}': {value}")]
    InvalidColor {
        theme_id: String,
        field: &'static str,
        value: String,
    },
}

fn normalize_theme_code(code: &str) -> String {
    match code.trim() {
        "Ropy Light" | "Light" | "light" => "ropy-light".to_string(),
        "Ropy Dark" | "Dark" | "dark" => "ropy-dark".to_string(),
        other => other.to_string(),
    }
}

#[derive(Debug, Deserialize)]
struct ThemeDefinitionRaw {
    theme_name: String,
    mode: ThemeMode,
    background: String,
    foreground: String,
    secondary: String,
    secondary_foreground: String,
    border: String,
    accent: String,
    accent_foreground: String,
    muted: String,
    muted_foreground: String,
    input: String,
    primary: String,
    primary_foreground: String,
    primary_hover: String,
    primary_active: String,
    danger: String,
    danger_foreground: String,
    popover: String,
    popover_foreground: String,
    selection: String,
    ring: String,
    list_hover: String,
    list_active: String,
    scrollbar_thumb: String,
}

fn parse_color(theme_id: &ThemeId, field: &'static str, value: &str) -> Result<u32, ThemeError> {
    let trimmed = value.trim();
    let normalized = trimmed.strip_prefix('#').map_or(trimmed, |hex| hex);
    if normalized.len() != 6 || !normalized.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(ThemeError::InvalidColor {
            theme_id: theme_id.code().to_string(),
            field,
            value: value.to_string(),
        });
    }

    u32::from_str_radix(normalized, 16).map_err(|_| ThemeError::InvalidColor {
        theme_id: theme_id.code().to_string(),
        field,
        value: value.to_string(),
    })
}

fn cached_display_names() -> &'static HashMap<String, String> {
    DISPLAY_NAMES.get_or_init(|| {
        ThemeAssets::iter()
            .filter_map(|name| {
                let theme_code = name.as_ref().strip_suffix(".toml")?.to_owned();
                let file = ThemeAssets::get(name.as_ref())?;
                let content = std::str::from_utf8(&file.data).ok()?;
                let theme_name = toml::from_str::<ThemeDefinitionRaw>(content)
                    .ok()?
                    .theme_name;

                Some((theme_code, theme_name))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;
    use serde::Deserialize;

    use super::{ThemeDefinition, ThemeError, ThemeId, ThemeMode, cached_display_names};

    #[test]
    fn test_theme_id_all_discovers_bundled_themes() {
        let ids = ThemeId::all();
        let codes: Vec<&str> = ids.iter().map(ThemeId::code).collect();

        assert!(codes.contains(&"ropy-dark"));
        assert!(codes.contains(&"ropy-light"));
        assert!(codes.contains(&"everforest-night"));
        assert!(codes.contains(&"nord-light"));
    }

    #[test]
    fn test_theme_id_display_name_returns_theme_name() {
        let theme = ThemeId::new("ropy-dark");

        assert_eq!(theme.display_name(), "Ropy Dark");
    }

    #[test]
    fn test_theme_display_name_cache_contains_bundled_themes() {
        let display_names = cached_display_names();

        assert_eq!(
            display_names.get("ropy-dark").map(String::as_str),
            Some("Ropy Dark")
        );
        assert_eq!(
            display_names.get("ropy-light").map(String::as_str),
            Some("Ropy Light")
        );
        assert_eq!(
            display_names.get("everforest-night").map(String::as_str),
            Some("Everforest Night")
        );
        assert_eq!(
            display_names.get("nord-light").map(String::as_str),
            Some("Nord Light")
        );
    }

    #[test]
    fn test_theme_display_name_cache_is_initialized_once() {
        assert!(std::ptr::eq(cached_display_names(), cached_display_names()));
    }

    #[test]
    fn test_theme_definition_load_bundled_ropy_dark_returns_palette() {
        let theme = ThemeDefinition::load(&ThemeId::new("ropy-dark")).unwrap();

        assert_eq!(theme.id.code(), "ropy-dark");
        assert_eq!(theme.name, "Ropy Dark");
        assert_eq!(theme.mode(), ThemeMode::Dark);
        assert_eq!(theme.palette().background, 0x002d_2d2d);
        assert_eq!(theme.palette().foreground, 0x00ff_ffff);
        assert_eq!(theme.palette().accent, 0x004d_4d4d);
        assert_eq!(theme.palette().primary, 0x006b_8cff);
    }

    #[test]
    fn test_theme_definition_load_bundled_ropy_light_returns_palette() {
        let theme = ThemeDefinition::load(&ThemeId::new("ropy-light")).unwrap();

        assert_eq!(theme.id.code(), "ropy-light");
        assert_eq!(theme.name, "Ropy Light");
        assert_eq!(theme.mode(), ThemeMode::Light);
        assert_eq!(theme.palette().background, 0x00ff_ffff);
        assert_eq!(theme.palette().accent, 0x00ad_d8e6);
        assert_eq!(theme.palette().primary, 0x005b_7bd5);
    }

    #[test]
    fn test_theme_definition_load_bundled_everforest_night_returns_palette() {
        let theme = ThemeDefinition::load(&ThemeId::new("everforest-night")).unwrap();

        assert_eq!(theme.id.code(), "everforest-night");
        assert_eq!(theme.name, "Everforest Night");
        assert_eq!(theme.mode(), ThemeMode::Dark);
        assert_eq!(theme.palette().background, 0x0023_2a2e);
        assert_eq!(theme.palette().primary, 0x00a7_c080);
    }

    #[test]
    fn test_theme_definition_load_bundled_nord_light_returns_palette() {
        let theme = ThemeDefinition::load(&ThemeId::new("nord-light")).unwrap();

        assert_eq!(theme.id.code(), "nord-light");
        assert_eq!(theme.name, "Nord Light");
        assert_eq!(theme.mode(), ThemeMode::Light);
        assert_eq!(theme.palette().background, 0x00e3_e7ee);
        assert_eq!(theme.palette().primary, 0x005e_81ac);
    }

    #[test]
    fn test_theme_definition_load_missing_theme_returns_not_found_error() {
        let result = ThemeDefinition::load(&ThemeId::new("missing-theme"));

        assert!(matches!(result, Err(ThemeError::NotFound(code)) if code == "missing-theme"));
    }

    #[derive(Debug, Deserialize)]
    struct ThemeConfig {
        theme: ThemeId,
    }

    #[rstest]
    #[case("theme = \"ropy-light\"", "ropy-light")]
    #[case("theme = \"ropy-dark\"", "ropy-dark")]
    #[case("theme = \"everforest-night\"", "everforest-night")]
    #[case("theme = \"nord-light\"", "nord-light")]
    fn test_theme_id_deserialize_bundled_code_roundtrips(
        #[case] toml_input: &str,
        #[case] expected: &str,
    ) {
        let config: ThemeConfig = toml::from_str(toml_input).unwrap();

        assert_eq!(config.theme.code(), expected);
    }

    #[rstest]
    #[case("theme = \"Light\"", "ropy-light")]
    #[case("theme = \"Dark\"", "ropy-dark")]
    fn test_theme_id_deserialize_legacy_value_maps_to_bundled_theme(
        #[case] toml_input: &str,
        #[case] expected: &str,
    ) {
        let config: ThemeConfig = toml::from_str(toml_input).unwrap();

        assert_eq!(config.theme.code(), expected);
    }
}

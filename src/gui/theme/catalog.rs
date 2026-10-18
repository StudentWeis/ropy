//! Validated theme snapshots. Only startup and explicit reload read user files.

use std::{collections::BTreeMap, path::Path, sync::OnceLock};

use gpui_kit::{App, Global};
use rust_i18n::t;

use super::{ThemeAssets, ThemeDefinition, ThemeError, ThemeId, ThemeMode};
use crate::config::Settings;

#[derive(Clone)]
#[expect(clippy::redundant_pub_crate)]
pub(crate) struct ThemeCatalog {
    themes: BTreeMap<String, (ThemeDefinition, String)>,
    pub(crate) diagnostics: Vec<String>,
}

impl Global for ThemeCatalog {}

impl Default for ThemeCatalog {
    fn default() -> Self {
        let themes = super::available_themes()
            .into_iter()
            .filter_map(|id| {
                let file = ThemeAssets::get(&format!("{}.toml", id.code()))?;
                let source = std::str::from_utf8(&file.data).ok()?.to_owned();
                let definition = ThemeDefinition::parse(&id, &source).ok()?;
                Some((id.code().to_owned(), (definition, source)))
            })
            .collect();
        Self {
            themes,
            diagnostics: Vec::new(),
        }
    }
}

impl ThemeCatalog {
    pub(crate) fn insert(&mut self, definition: ThemeDefinition, source: String) {
        self.themes
            .insert(definition.id().code().to_owned(), (definition, source));
    }

    pub(crate) fn global(cx: &App) -> &Self {
        // Test and promotional windows use bundled themes without accessing user files.
        static BUNDLED: OnceLock<ThemeCatalog> = OnceLock::new();
        cx.try_global::<Self>()
            .unwrap_or_else(|| BUNDLED.get_or_init(Self::default))
    }

    pub(crate) fn definitions(&self) -> impl Iterator<Item = &ThemeDefinition> {
        self.themes.values().map(|(definition, _)| definition)
    }

    pub(crate) fn get(&self, id: &ThemeId) -> Option<&ThemeDefinition> {
        self.themes.get(id.code()).map(|(definition, _)| definition)
    }

    pub(crate) fn source(&self, id: &ThemeId) -> Option<&str> {
        self.themes
            .get(id.code())
            .map(|(_, source)| source.as_str())
    }

    pub(crate) fn for_mode(&self, selected: &ThemeId, mode: ThemeMode) -> ThemeId {
        if self.get(selected).is_some_and(|theme| theme.mode() == mode) {
            selected.clone()
        } else {
            ThemeId::new(match mode {
                ThemeMode::Light => "ropy-light",
                ThemeMode::Dark => "ropy-dark",
            })
        }
    }

    pub(crate) fn resolve(&self, settings: &Settings, system_mode: ThemeMode) -> ThemeId {
        if !settings.appearance.follow_system {
            return if self.get(&settings.theme).is_some() {
                settings.theme.clone()
            } else {
                self.for_mode(&settings.theme, system_mode)
            };
        }
        let selected = match system_mode {
            ThemeMode::Light => &settings.appearance.light_theme,
            ThemeMode::Dark => &settings.appearance.dark_theme,
        };
        self.for_mode(selected, system_mode)
    }

    pub(crate) fn set_follow_system(
        &self,
        settings: &mut Settings,
        enabled: bool,
        mode: ThemeMode,
    ) {
        if settings.appearance.follow_system && !enabled {
            settings.theme = self.resolve(settings, mode);
        }
        settings.appearance.follow_system = enabled;
    }

    /// Keep previously valid files on read/parse failure or deletion, until restart.
    pub(crate) fn reload(mut self, directory: &Path) -> Self {
        self.diagnostics.clear();
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                for id in self.themes.keys().filter(|id| id.starts_with("user:")) {
                    self.diagnostics
                        .push(t!("themes_missing", theme = id).to_string());
                }
                return self;
            }
            Err(error) => {
                tracing::warn!(%error, path = %directory.display(), "cannot read theme directory");
                self.diagnostics
                    .push(t!("themes_directory_failed", path = directory.display()).to_string());
                return self;
            }
        };
        let mut seen = std::collections::BTreeSet::new();
        let mut paths = Vec::new();
        for entry in entries {
            match entry {
                Ok(entry) => paths.push(entry.path()),
                Err(error) => {
                    tracing::warn!(%error, "cannot read theme entry");
                    self.diagnostics.push(
                        t!("themes_directory_failed", path = directory.display()).to_string(),
                    );
                }
            }
        }
        paths.sort();
        for path in paths {
            if path.extension().is_none_or(|extension| extension != "toml") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|name| name.to_str()) else {
                continue;
            };
            if stem.is_empty()
                || !stem
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
            {
                self.diagnostics
                    .push(t!("themes_invalid_filename", file = path.display()).to_string());
                continue;
            }
            let id = ThemeId::new(format!("user:{stem}"));
            seen.insert(id.code().to_owned());
            let source = match std::fs::read_to_string(&path) {
                Ok(source) => source,
                Err(error) => {
                    tracing::warn!(%error, path = %path.display(), "cannot read user theme");
                    self.diagnostics
                        .push(t!("themes_read_failed", file = path.display()).to_string());
                    continue;
                }
            };
            match ThemeDefinition::parse(&id, &source) {
                Ok(definition) => {
                    self.themes
                        .insert(id.code().to_owned(), (definition, source));
                }
                Err(error) => {
                    tracing::warn!(%error, path = %path.display(), "invalid user theme");
                    let reason = match error {
                        ThemeError::InvalidColor { field, value, .. } => {
                            t!("themes_invalid_color", field = field, value = value).to_string()
                        }
                        ThemeError::EmptyName => t!("themes_empty_name").to_string(),
                        ThemeError::ParseToml { source, .. } => {
                            t!("themes_invalid_toml", detail = source.message()).to_string()
                        }
                        _ => t!("themes_read_failed", file = path.display()).to_string(),
                    };
                    self.diagnostics.push(
                        t!(
                            "themes_invalid_file",
                            file = path.display(),
                            reason = reason
                        )
                        .to_string(),
                    );
                }
            }
        }
        for id in self
            .themes
            .keys()
            .filter(|id| id.starts_with("user:") && !seen.contains(*id))
        {
            self.diagnostics
                .push(t!("themes_missing", theme = id).to_string());
        }
        self
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    const DARK: &str = include_str!("../../../assets/themes/ropy-dark.toml");

    #[test]
    fn test_catalog_user_theme_keeps_builtin_identity_and_resolves_appearance() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(
            directory.path().join("ropy-dark.toml"),
            DARK.replace("#2d2d2d", "#123456"),
        )
        .unwrap();
        let catalog = ThemeCatalog::default().reload(directory.path());
        let id = ThemeId::new("user:ropy-dark");
        assert_eq!(catalog.get(&id).unwrap().palette().background, 0x0012_3456);
        assert_eq!(
            catalog
                .get(&ThemeId::new("ropy-dark"))
                .unwrap()
                .palette()
                .background,
            0x002d_2d2d
        );
        assert_eq!(catalog.for_mode(&id, ThemeMode::Dark), id);
        assert_eq!(catalog.for_mode(&id, ThemeMode::Light).code(), "ropy-light");
        let mut settings = Settings::default();
        settings.theme = id.clone();
        assert_eq!(catalog.resolve(&settings, ThemeMode::Light), id);
        settings.appearance.follow_system = true;
        settings.appearance.dark_theme = id.clone();
        assert_eq!(catalog.resolve(&settings, ThemeMode::Dark), id);
        assert_eq!(
            catalog.resolve(&settings, ThemeMode::Light).code(),
            "ropy-light"
        );
    }

    #[test]
    fn test_catalog_reload_replaces_valid_content_and_retains_invalid_or_missing_content() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("night.toml");
        let id = ThemeId::new("user:night");
        std::fs::write(&path, DARK).unwrap();
        let catalog = ThemeCatalog::default().reload(directory.path());
        std::fs::write(
            &path,
            DARK.replace("Ropy Dark", "New name")
                .replace("#2d2d2d", "#123456"),
        )
        .unwrap();
        let catalog = catalog.reload(directory.path());
        assert_eq!(catalog.get(&id).unwrap().name(), "New name");
        assert_eq!(catalog.get(&id).unwrap().palette().background, 0x0012_3456);
        let source = catalog.source(&id).unwrap().to_owned();
        std::fs::write(&path, "not valid TOML").unwrap();
        let catalog = catalog.reload(directory.path());
        assert_eq!(catalog.source(&id), Some(source.as_str()));
        assert_eq!(catalog.diagnostics.len(), 1);
        std::fs::remove_file(&path).unwrap();
        let catalog = catalog.reload(directory.path());
        assert_eq!(catalog.source(&id), Some(source.as_str()));
        assert_eq!(catalog.diagnostics.len(), 1);
        // A fresh process has no last valid copy and must fall back without rewriting the choice.
        let fresh = ThemeCatalog::default().reload(directory.path());
        let mut settings = Settings::default();
        settings.theme = id.clone();
        assert_eq!(
            fresh.resolve(&settings, ThemeMode::Dark).code(),
            "ropy-dark"
        );
        assert_eq!(settings.theme, id);
    }

    #[test]
    fn test_catalog_bad_files_do_not_hide_valid_files_or_read_nested_paths() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("good.toml"), DARK).unwrap();
        std::fs::write(directory.path().join("broken.toml"), "broken").unwrap();
        std::fs::write(directory.path().join("Wrong.toml"), DARK).unwrap();
        std::fs::create_dir(directory.path().join("nested")).unwrap();
        std::fs::write(directory.path().join("nested/hidden.toml"), DARK).unwrap();
        let catalog = ThemeCatalog::default().reload(directory.path());
        assert!(catalog.get(&ThemeId::new("user:good")).is_some());
        assert!(catalog.get(&ThemeId::new("user:broken")).is_none());
        assert!(catalog.get(&ThemeId::new("user:Wrong")).is_none());
        assert!(catalog.get(&ThemeId::new("user:nested/hidden")).is_none());
        assert_eq!(catalog.diagnostics.len(), 2);
        let catalog = catalog.reload(&directory.path().join("good.toml"));
        assert!(catalog.get(&ThemeId::new("user:good")).is_some());
        assert_eq!(catalog.diagnostics.len(), 1);
    }
}

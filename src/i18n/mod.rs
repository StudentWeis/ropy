use std::collections::HashMap;

use language::LocaleAssets;

mod error;
mod language;
mod translations;

pub use error::I18nError;
pub use language::Language;
pub use translations::Translations;

/// I18n manager for handling translations.
#[derive(Debug, Clone)]
pub struct I18n {
    current_language: Language,
    translations: Translations,
}

impl I18n {
    /// Create a new `I18n` instance for the given language.
    ///
    /// If the locale file for `language` is not found, falls back to the
    /// first language returned by [`Language::all()`].
    pub fn new(language: Language) -> Result<Self, I18nError> {
        let translations = Self::load_language(&language)?;
        Ok(Self {
            current_language: language,
            translations,
        })
    }

    /// Load translations for a specific language from the embedded assets.
    fn load_language(language: &Language) -> Result<Translations, I18nError> {
        let file_name = format!("{}.toml", language.code());
        let file = LocaleAssets::get(&file_name).or_else(|| {
            // Fall back to the first available locale when the requested one
            // is missing (e.g. after a locale file was removed).
            Language::all()
                .into_iter()
                .next()
                .and_then(|first| LocaleAssets::get(&format!("{}.toml", first.code())))
        });

        let data = file.ok_or_else(|| {
            I18nError::NotFound(format!("no locale file found for '{}'", language.code()))
        })?;

        let content =
            std::str::from_utf8(&data.data).map_err(|e| I18nError::ParseError(e.to_string()))?;

        Translations::from_toml(content)
    }

    /// Change the current language.
    pub fn set_language(&mut self, language: Language) -> Result<(), I18nError> {
        let translations = Self::load_language(&language)?;
        self.current_language = language;
        self.translations = translations;
        Ok(())
    }

    /// Get a translated string by key.
    pub fn t(&self, key: &str) -> String {
        self.translations.get(key)
    }
}

impl Default for I18n {
    fn default() -> Self {
        match Self::new(Language::default()) {
            Ok(i18n) => i18n,
            Err(e) => {
                tracing::warn!(
                    error = ?e,
                    "failed to load default language translations; falling back to empty translations"
                );
                Self {
                    current_language: Language::default(),
                    translations: Translations {
                        strings: HashMap::new(),
                    },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_display_name() {
        assert_eq!(Language::new("en").display_name(), "English");
        assert_eq!(Language::new("zh-CN").display_name(), "简体中文");
        assert_eq!(Language::new("ja").display_name(), "日本語");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_translations_from_toml() {
        let content = r#"
            show = "Show"
            quit = "Quit"
            on = "ON"
            off = "OFF"
        "#;
        let translations = Translations::from_toml(content).unwrap();
        assert_eq!(translations.get("show"), "Show");
        assert_eq!(translations.get("quit"), "Quit");
        assert_eq!(translations.get("on"), "ON");
        assert_eq!(translations.get("off"), "OFF");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_missing_translation() {
        let content = r#"
            foo = "bar"
        "#;
        let translations = Translations::from_toml(content).unwrap();
        assert_eq!(translations.get("missing_key"), "[Missing: missing_key]");
        assert_eq!(translations.get("on"), "[Missing: on]");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_i18n_initialization() {
        let i18n = I18n::new(Language::new("en"));
        assert!(i18n.is_ok());
        let i18n = i18n.unwrap();
        assert_eq!(i18n.t("tray_show"), "Show");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_i18n_language_switch() {
        let mut i18n = I18n::new(Language::new("en")).unwrap();
        assert_eq!(i18n.t("tray_show"), "Show");

        // Switch to Chinese
        let result = i18n.set_language(Language::new("zh-CN"));
        assert!(result.is_ok());
        assert_eq!(i18n.t("tray_show"), "显示");
    }

    #[test]
    fn test_language_all() {
        let languages = Language::all();
        // Must contain at least the three bundled locales
        assert!(languages.len() >= 3);
        assert!(languages.iter().any(|l| l.code() == "en"));
        assert!(languages.iter().any(|l| l.code() == "zh-CN"));
        assert!(languages.iter().any(|l| l.code() == "ja"));
        // Must be sorted by locale code
        let codes: Vec<&str> = languages.iter().map(Language::code).collect();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        assert_eq!(codes, sorted);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_unknown_language_fallback() {
        // An unknown locale should fall back to the first available locale
        let i18n = I18n::new(Language::new("xx-UNKNOWN"));
        assert!(i18n.is_ok());
    }
}

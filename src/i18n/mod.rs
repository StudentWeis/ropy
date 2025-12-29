use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(rename = "en")]
    English,
    #[serde(rename = "zh-CN")]
    ChineseSimplified,
}

impl Language {
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::ChineseSimplified => "zh-CN",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::ChineseSimplified => "简体中文",
        }
    }

    pub fn all() -> Vec<Language> {
        vec![Language::English, Language::ChineseSimplified]
    }
}

impl Default for Language {
    fn default() -> Self {
        Language::English
    }
}

/// Translation keys used throughout the application
#[derive(Debug, Clone)]
pub struct Translations {
    strings: HashMap<String, String>,
}

impl Translations {
    /// Load translations from a TOML string
    pub fn from_toml(content: &str) -> Result<Self, I18nError> {
        let strings: HashMap<String, String> =
            toml::from_str(content).map_err(|e| I18nError::ParseError(e.to_string()))?;
        Ok(Self { strings })
    }

    /// Get a translated string by key
    pub fn get(&self, key: &str) -> String {
        self.strings
            .get(key)
            .cloned()
            .unwrap_or_else(|| format!("[Missing: {}]", key))
    }
}

/// I18n manager for handling translations
#[derive(Debug, Clone)]
pub struct I18n {
    current_language: Language,
    translations: Translations,
}

impl I18n {
    /// Create a new I18n instance with the specified language
    pub fn new(language: Language) -> Result<Self, I18nError> {
        let translations = Self::load_language(language)?;
        Ok(Self {
            current_language: language,
            translations,
        })
    }

    /// Load translations for a specific language
    fn load_language(language: Language) -> Result<Translations, I18nError> {
        let content = match language {
            Language::English => include_str!("../../assets/locales/en.toml"),
            Language::ChineseSimplified => include_str!("../../assets/locales/zh-CN.toml"),
        };
        Translations::from_toml(content)
    }

    /// Get the current language
    pub fn current_language(&self) -> Language {
        self.current_language
    }

    /// Change the current language
    pub fn set_language(&mut self, language: Language) -> Result<(), I18nError> {
        let translations = Self::load_language(language)?;
        self.current_language = language;
        self.translations = translations;
        Ok(())
    }

    /// Get a translated string by key
    pub fn t(&self, key: &str) -> String {
        self.translations.get(key)
    }
}

impl Default for I18n {
    fn default() -> Self {
        // Try to load English as default, if that fails, create empty translations
        match Self::new(Language::default()) {
            Ok(i18n) => i18n,
            Err(e) => {
                eprintln!("[ropy] Warning: Failed to load default language translations: {e}");
                eprintln!("[ropy] Falling back to empty translations - all strings will show as '[Missing: key]'");
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

/// I18n-related errors
#[derive(Debug, Error)]
pub enum I18nError {
    #[error("Failed to parse translation file: {0}")]
    ParseError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_code() {
        assert_eq!(Language::English.code(), "en");
        assert_eq!(Language::ChineseSimplified.code(), "zh-CN");
    }

    #[test]
    fn test_language_display_name() {
        assert_eq!(Language::English.display_name(), "English");
        assert_eq!(Language::ChineseSimplified.display_name(), "简体中文");
    }

    #[test]
    fn test_translations_from_toml() {
        let content = r#"
            app_name = "Ropy"
            show = "Show"
            quit = "Quit"
        "#;
        let translations = Translations::from_toml(content).unwrap();
        assert_eq!(translations.get("app_name"), "Ropy");
        assert_eq!(translations.get("show"), "Show");
        assert_eq!(translations.get("quit"), "Quit");
    }

    #[test]
    fn test_missing_translation() {
        let content = r#"
            app_name = "Ropy"
        "#;
        let translations = Translations::from_toml(content).unwrap();
        assert_eq!(translations.get("missing_key"), "[Missing: missing_key]");
    }

    #[test]
    fn test_i18n_initialization() {
        let i18n = I18n::new(Language::English);
        assert!(i18n.is_ok());
        let i18n = i18n.unwrap();
        assert_eq!(i18n.current_language(), Language::English);
        assert_eq!(i18n.t("app_name"), "Ropy");
    }

    #[test]
    fn test_i18n_language_switch() {
        let mut i18n = I18n::new(Language::English).unwrap();
        assert_eq!(i18n.t("tray_show"), "Show");

        // Switch to Chinese
        let result = i18n.set_language(Language::ChineseSimplified);
        assert!(result.is_ok());
        assert_eq!(i18n.current_language(), Language::ChineseSimplified);
        assert_eq!(i18n.t("tray_show"), "显示");
    }

    #[test]
    fn test_language_all() {
        let languages = Language::all();
        assert_eq!(languages.len(), 2);
        assert!(languages.contains(&Language::English));
        assert!(languages.contains(&Language::ChineseSimplified));
    }
}

use std::collections::HashMap;

use super::error::I18nError;

/// Translation keys used throughout the application.
#[derive(Debug, Clone)]
pub struct Translations {
    pub(super) strings: HashMap<String, String>,
}

impl Translations {
    /// Load translations from a `TOML` string.
    pub fn from_toml(content: &str) -> Result<Self, I18nError> {
        let strings: HashMap<String, String> =
            toml::from_str(content).map_err(|e| I18nError::ParseError(e.to_string()))?;
        Ok(Self { strings })
    }

    /// Get a translated string by key.
    pub fn get(&self, key: &str) -> String {
        self.strings
            .get(key)
            .cloned()
            .unwrap_or_else(|| format!("[Missing: {key}]"))
    }
}

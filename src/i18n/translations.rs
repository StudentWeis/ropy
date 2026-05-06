use std::collections::HashMap;

use super::error::I18nError;

#[derive(Debug, Clone)]
pub struct Translations {
    pub(super) strings: HashMap<String, String>,
}

impl Translations {
    pub fn from_toml(content: &str) -> Result<Self, I18nError> {
        let strings: HashMap<String, String> =
            toml::from_str(content).map_err(|e| I18nError::ParseError(e.to_string()))?;
        Ok(Self { strings })
    }

    /// Returns the translation for `key`, or a visible `[Missing: key]`
    /// placeholder so missing translations surface in the UI instead of
    /// silently rendering as an empty string.
    pub fn get(&self, key: &str) -> String {
        self.strings
            .get(key)
            .cloned()
            .unwrap_or_else(|| format!("[Missing: {key}]"))
    }
}

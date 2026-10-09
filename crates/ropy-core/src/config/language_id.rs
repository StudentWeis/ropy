//! Persisted locale identifiers.

use serde::{Deserialize, Serialize};

/// A language identified by its locale code (e.g. `"en"`, `"zh-CN"`).
///
/// Serializes / deserializes transparently as the locale code string, keeping
/// existing `config.toml` files fully compatible.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Language(String);

impl Language {
    /// Create a language from a locale code string.
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    /// Return the locale code (e.g. `"en"`, `"zh-CN"`).
    #[must_use]
    pub fn code(&self) -> &str {
        &self.0
    }
}

impl Default for Language {
    fn default() -> Self {
        Self::new("en")
    }
}

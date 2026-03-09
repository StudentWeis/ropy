use std::collections::HashMap;

use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};

/// Embedded locale TOML files from `assets/locales/`.
/// Adding a new `<code>.toml` file to that directory automatically makes the
/// language available after recompilation — no Rust source changes needed.
#[derive(RustEmbed)]
#[folder = "assets/locales"]
pub(super) struct LocaleAssets;

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
    pub fn code(&self) -> &str {
        &self.0
    }

    /// Return the human-readable display name read from the `language_name`
    /// key inside the corresponding TOML file.  Falls back to the locale code
    /// if the file or key is absent.
    pub fn display_name(&self) -> String {
        let file_name = format!("{}.toml", self.0);
        if let Some(file) = LocaleAssets::get(&file_name)
            && let Ok(content) = std::str::from_utf8(&file.data)
            && let Ok(map) = toml::from_str::<HashMap<String, String>>(content)
            && let Some(name) = map.get("language_name")
        {
            return name.clone();
        }
        self.0.clone()
    }

    /// Return all languages discovered from `assets/locales/*.toml`, sorted
    /// alphabetically by locale code.  No code change is required when new
    /// TOML files are added to the directory.
    pub fn all() -> Vec<Self> {
        let mut codes: Vec<String> = LocaleAssets::iter()
            .filter_map(|name| name.as_ref().strip_suffix(".toml").map(str::to_owned))
            .collect();
        codes.sort();
        codes.into_iter().map(Self).collect()
    }
}

impl Default for Language {
    fn default() -> Self {
        Self::new("en")
    }
}

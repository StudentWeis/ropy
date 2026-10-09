//! Persisted theme identifiers, including legacy aliases.

use serde::{Deserialize, Deserializer, Serialize};

/// Theme identifier — the bundle file name without the `.toml` suffix.
/// Serialized transparently as the raw string so existing `config.toml`
/// values keep working.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ThemeId(String);

impl ThemeId {
    /// Normalize a theme code, accepting legacy light/dark aliases.
    pub fn new(code: impl Into<String>) -> Self {
        Self(normalize_theme_code(&code.into()))
    }

    /// Return the canonical serialized theme code.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.0
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

fn normalize_theme_code(code: &str) -> String {
    match code.trim() {
        "Ropy Light" | "Light" | "light" => "ropy-light".to_string(),
        "Ropy Dark" | "Dark" | "dark" => "ropy-dark".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use rstest::rstest;
    use serde::Deserialize;

    use super::ThemeId;

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

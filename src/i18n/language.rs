use std::{collections::HashMap, sync::OnceLock};

use rust_embed::RustEmbed;

/// Embedded locale TOML files from `assets/locales/`.
/// Adding a new `<code>.toml` file to that directory automatically makes the
/// language available after recompilation — no Rust source changes needed.
#[derive(RustEmbed)]
#[folder = "assets/locales"]
pub(super) struct LocaleAssets;

static DISPLAY_NAMES: OnceLock<HashMap<String, String>> = OnceLock::new();

pub(crate) use crate::config::language_id::Language;

pub(crate) fn language_display_name(language: &Language) -> String {
    cached_display_names()
        .get(language.code())
        .cloned()
        .unwrap_or_else(|| language.code().to_owned())
}

pub(crate) fn available_languages() -> Vec<Language> {
    let mut codes: Vec<String> = LocaleAssets::iter()
        .filter_map(|name| name.as_ref().strip_suffix(".toml").map(str::to_owned))
        .collect();
    codes.sort();
    codes.into_iter().map(Language::new).collect()
}

fn cached_display_names() -> &'static HashMap<String, String> {
    DISPLAY_NAMES.get_or_init(|| {
        LocaleAssets::iter()
            .filter_map(|name| {
                let locale_code = name.as_ref().strip_suffix(".toml")?.to_owned();
                let file = LocaleAssets::get(name.as_ref())?;
                let content = std::str::from_utf8(&file.data).ok()?;
                let name = toml::from_str::<HashMap<String, String>>(content)
                    .ok()?
                    .get("language_name")
                    .cloned()?;

                Some((locale_code, name))
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_name_cache_contains_bundled_languages() {
        let display_names = cached_display_names();

        assert_eq!(display_names.get("en").map(String::as_str), Some("English"));
        assert_eq!(
            display_names.get("zh-CN").map(String::as_str),
            Some("简体中文")
        );
        assert_eq!(display_names.get("ja").map(String::as_str), Some("日本語"));
    }

    #[test]
    fn test_display_name_cache_is_initialized_once() {
        assert!(std::ptr::eq(cached_display_names(), cached_display_names()));
    }
}

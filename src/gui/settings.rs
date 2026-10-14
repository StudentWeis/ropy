//! GPUI owns the live settings snapshot; the config module owns its format.

use std::str::FromStr;

use gpui_kit::{App, BorrowAppContext, Global, ReadGlobal, SharedString};
use rust_i18n::t;

use crate::config::{LayoutMode, Settings, language_id::Language};

#[derive(Debug, Clone)]
pub(crate) struct GlobalSettings(Settings);

impl Global for GlobalSettings {}

impl GlobalSettings {
    pub(crate) const fn new(settings: Settings) -> Self {
        Self(settings)
    }

    pub(crate) fn read<R>(cx: &App, reader: impl FnOnce(&Settings) -> R) -> R {
        reader(&Self::global(cx).0)
    }

    pub(crate) fn update<R>(cx: &mut App, update: impl FnOnce(&mut Settings) -> R) -> R {
        cx.update_global::<Self, _>(|global, _| update(&mut global.0))
    }
}

pub(crate) fn layout_label(mode: LayoutMode) -> SharedString {
    let label = match mode {
        LayoutMode::List => t!("settings_layout_list"),
        LayoutMode::Grid => t!("settings_layout_grid"),
    };
    SharedString::from(label)
}

/// Bundled languages in stable locale-code order for the settings selector.
pub(crate) fn available_languages() -> Vec<Language> {
    let mut languages: Vec<_> = rust_i18n::available_locales!()
        .into_iter()
        .map(|locale| Language::new(locale.into_owned()))
        .collect();
    languages.sort_by(|left, right| left.code().cmp(right.code()));
    languages
}

/// Show each language in its own language, independent of the active locale.
pub(crate) fn language_display_name(language: &Language) -> SharedString {
    t!("language_name", locale = language.code()).into()
}

/// Validate hotkey and reset to default if invalid.
pub(crate) fn validate_hotkey(settings: &mut Settings) {
    if settings.hotkey.activation_key.is_empty()
        || global_hotkey::hotkey::HotKey::from_str(&settings.hotkey.activation_key).is_err()
    {
        settings.hotkey.activation_key = Settings::default().hotkey.activation_key;
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case("en", "Settings", "English")]
    #[case("zh-CN", "设置", "简体中文")]
    #[case("ja", "設定", "日本語")]
    #[case("ja-JP", "設定", "日本語")]
    #[case("xx-UNKNOWN", "Settings", "English")]
    fn test_translation_locale_resolves_with_fallback(
        #[case] locale: &str,
        #[case] settings: &str,
        #[case] name: &str,
    ) {
        assert_eq!(t!("tray_settings", locale = locale), settings);
        assert_eq!(language_display_name(&Language::new(locale)).as_ref(), name);
    }

    #[rstest]
    #[case("en", "3 files")]
    #[case("zh-CN", "3 个文件")]
    #[case("ja", "3 個のファイル")]
    fn test_translation_count_interpolates_named_parameter(
        #[case] locale: &str,
        #[case] expected: &str,
    ) {
        assert_eq!(
            t!("record_file_count", locale = locale, count = 3),
            expected
        );
    }

    #[rstest]
    #[case(
        "en",
        "Version 1.2.3",
        "Downloading… 42%",
        "Failed to save settings: disk full"
    )]
    #[case("zh-CN", "版本 1.2.3", "下载中… 42%", "保存设置失败：disk full")]
    #[case(
        "ja",
        "バージョン 1.2.3",
        "ダウンロード中… 42%",
        "設定の保存に失敗しました：disk full"
    )]
    fn test_translation_messages_own_parameter_order_and_punctuation(
        #[case] locale: &str,
        #[case] version: &str,
        #[case] progress: &str,
        #[case] failure: &str,
    ) {
        assert_eq!(
            t!("about_version", locale = locale, version = "1.2.3"),
            version
        );
        assert_eq!(
            t!("update_downloading", locale = locale, progress = 41.6 : {:.0}),
            progress
        );
        assert_eq!(
            t!("settings_save_failed", locale = locale, error = "disk full"),
            failure
        );
    }

    #[test]
    fn test_available_languages_matches_bundled_catalogues_in_code_order() {
        let languages = available_languages();
        let codes: Vec<_> = languages.iter().map(Language::code).collect();
        for required in ["en", "ja", "zh-CN"] {
            assert!(codes.contains(&required));
        }
        assert!(codes.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    #[expect(clippy::expect_used)]
    fn test_language_switch_updates_application_components_and_other_threads() {
        // rust-i18n's locale is process-wide. Run mutations in a child test
        // process so concurrent GPUI tests cannot observe another language.
        const CHILD: &str = "ROPY_TEST_LOCALE_SWITCH";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
                .args([
                    "--exact",
                    "gui::settings::tests::test_language_switch_updates_application_components_and_other_threads",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .expect("run isolated locale test");
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        for (locale, expected) in [("zh-CN", "设置"), ("ja", "設定"), ("en", "Settings")] {
            rust_i18n::set_locale(locale);
            assert_eq!(&*gpui_kit::component::locale(), locale);
            assert_eq!(t!("tray_settings"), expected);
            assert_eq!(
                layout_label(LayoutMode::List).as_ref(),
                t!("settings_layout_list", locale = locale).as_ref()
            );
            assert_eq!(
                std::thread::spawn(|| t!("tray_settings").into_owned())
                    .join()
                    .expect("translation thread"),
                expected
            );
            // Explicit locale queries must not change the application locale.
            assert_eq!(t!("tray_settings", locale = "en"), "Settings");
            assert_eq!(&*rust_i18n::locale(), locale);
        }
    }

    // ── Hotkey Validation Tests ───────────────────────────────────

    #[test]
    fn test_validate_hotkey_valid() {
        let mut settings = Settings::default();
        settings.hotkey.activation_key = "ctrl+shift+v".to_string();

        validate_hotkey(&mut settings);

        // Valid hotkey should not be changed
        assert_eq!(settings.hotkey.activation_key, "ctrl+shift+v");
    }

    #[test]
    fn test_validate_hotkey_empty() {
        let mut settings = Settings::default();
        settings.hotkey.activation_key = String::new();

        validate_hotkey(&mut settings);

        // Empty hotkey should be reset to default
        let default = Settings::default();
        assert_eq!(
            settings.hotkey.activation_key,
            default.hotkey.activation_key
        );
    }

    #[test]
    fn test_validate_hotkey_invalid() {
        let mut settings = Settings::default();
        settings.hotkey.activation_key = "not+a+valid+hotkey".to_string();

        validate_hotkey(&mut settings);

        // Invalid hotkey should be reset to default
        let default = Settings::default();
        assert_eq!(
            settings.hotkey.activation_key,
            default.hotkey.activation_key
        );
    }

    #[test]
    fn test_validate_hotkey_gibberish() {
        let mut settings = Settings::default();
        settings.hotkey.activation_key = "@@@###".to_string();

        validate_hotkey(&mut settings);

        let default = Settings::default();
        assert_eq!(
            settings.hotkey.activation_key,
            default.hotkey.activation_key
        );
    }

    #[test]
    fn test_hotkey_settings_default() {
        let hotkey = Settings::default().hotkey;
        // Default hotkey should be valid
        assert_ne!(hotkey.activation_key, "");
        // Verify it's a valid hotkey string
        assert!(global_hotkey::hotkey::HotKey::from_str(&hotkey.activation_key).is_ok());
    }
}

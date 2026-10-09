//! GPUI owns the live settings snapshot; the config module owns its format.

use std::str::FromStr;

use gpui_kit::{App, BorrowAppContext, Global, ReadGlobal, SharedString};

use crate::{
    config::{LayoutMode, Settings},
    i18n::I18n,
};

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

pub(crate) fn layout_label(mode: LayoutMode, cx: &App) -> SharedString {
    let label = match mode {
        LayoutMode::List => I18n::translate(cx, "settings_layout_list"),
        LayoutMode::Grid => I18n::translate(cx, "settings_layout_grid"),
    };
    SharedString::from(label)
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
    use super::*;
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

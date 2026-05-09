//! Update-check settings handlers (`auto_check`, `include_prerelease`).

use gpui::{Context, Window};

use super::RopyBoard;
use crate::{config::Settings, i18n::I18n};

impl RopyBoard {
    pub(crate) fn save_auto_check_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let previous_value = Settings::read(cx, |s| s.update.auto_check);
        if enabled == previous_value {
            return;
        }

        if let Err(error_message) = Self::persist_settings_update(cx, |settings| {
            settings.update.auto_check = enabled;
        }) {
            Self::notify_settings_save_failed(window, cx, &error_message);
            return;
        }

        self.settings_editor.update_settings.auto_check_enabled = enabled;
        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }

    pub(crate) fn save_include_prerelease_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let previous_value = Settings::read(cx, |s| s.update.include_prerelease);
        if enabled == previous_value {
            return;
        }

        if let Err(error_message) = Self::persist_settings_update(cx, |settings| {
            settings.update.include_prerelease = enabled;
        }) {
            Self::notify_settings_save_failed(window, cx, &error_message);
            return;
        }

        self.settings_editor
            .update_settings
            .include_prerelease_enabled = enabled;
        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }
}

use std::str::FromStr;

use gpui::{BorrowAppContext, Context, Window, prelude::Styled, px};
use gpui_component::{WindowExt, notification::Notification};

use super::{RopyBoard, settings_editor};
use crate::{
    config::{ConfirmMode, Settings},
    i18n::I18n,
};

mod hotkey;
mod layout;
mod storage;
mod theme;
mod update;

const SETTINGS_NOTIFICATION_MAX_WIDTH_PX: f32 = 280.0;

impl RopyBoard {
    fn push_settings_notification(
        window: &mut Window,
        notification: Notification,
        cx: &mut Context<'_, Self>,
    ) {
        window.push_notification(
            notification
                .w_auto()
                .max_w(px(SETTINGS_NOTIFICATION_MAX_WIDTH_PX)),
            cx,
        );
    }

    fn notify_settings_warning(
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        message: impl Into<gpui::SharedString>,
    ) {
        Self::push_settings_notification(window, Notification::warning(message.into()), cx);
    }

    fn notify_settings_success(
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        message: impl Into<gpui::SharedString>,
    ) {
        Self::push_settings_notification(window, Notification::success(message.into()), cx);
    }

    fn notify_settings_save_failed(
        window: &mut Window,
        cx: &mut Context<'_, Self>,
        error_message: &str,
    ) {
        let message = format!(
            "{}: {}",
            I18n::translate(cx, "settings_save_failed"),
            error_message
        );
        Self::push_settings_notification(window, Notification::error(message), cx);
    }

    fn persist_settings_update(
        cx: &mut Context<'_, Self>,
        updater: impl FnOnce(&mut Settings),
    ) -> Result<(), String> {
        let mut result = Ok(());
        let mut updater = Some(updater);

        cx.update_global::<Settings, _>(|settings, _cx| {
            let previous = settings.clone();

            if let Some(updater) = updater.take() {
                updater(settings);
            }

            if let Err(error) = settings.save() {
                tracing::warn!(error = %error, "failed to save settings");
                *settings = previous;
                result = Err(format!("{error}"));
            }
        });

        result
    }

    fn set_theme_selection(
        &mut self,
        theme_idx: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.settings_editor.selected_theme = theme_idx;
        self.settings_editor.theme_select.update(cx, |state, cx| {
            state.set_selected_index(
                Some(gpui_component::IndexPath::default().row(theme_idx)),
                window,
                cx,
            );
        });
    }

    fn set_language_selection(
        &mut self,
        language_idx: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.settings_editor.selected_language = language_idx;
        self.settings_editor
            .language_select
            .update(cx, |state, cx| {
                state.set_selected_index(
                    Some(gpui_component::IndexPath::default().row(language_idx)),
                    window,
                    cx,
                );
            });
    }

    fn set_layout_selection(
        &mut self,
        layout_idx: usize,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.settings_editor.selected_layout = layout_idx;
        self.sync_layout_select_items(window, cx);
    }

    pub(crate) fn sync_layout_select_items(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        settings_editor::sync_layout_select_items(
            &self.settings_editor.layout_select,
            self.settings_editor.selected_layout,
            window,
            cx,
        );
    }

    pub(crate) fn toggle_autostart(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let previous_value = Settings::read(cx, |s| s.autostart.enabled);
        let next_value = !self.settings_editor.autostart.enabled;

        if let Err(error_message) = Self::persist_settings_update(cx, |settings| {
            settings.autostart.enabled = next_value;
        }) {
            Self::notify_settings_save_failed(window, cx, &error_message);
            return;
        }

        if let Err(error) = Self::sync_autostart_state_for(next_value) {
            tracing::warn!(error = ?error, "failed to sync auto-start state");
            if let Err(rollback_error) = Self::persist_settings_update(cx, |settings| {
                settings.autostart.enabled = previous_value;
            }) {
                Self::notify_settings_save_failed(window, cx, &rollback_error);
            }
            self.settings_editor.autostart.enabled = Settings::read(cx, |s| s.autostart.enabled);
            Self::notify_settings_warning(
                window,
                cx,
                I18n::translate(cx, "settings_autostart_failed"),
            );
            cx.notify();
            return;
        }

        self.settings_editor.autostart.enabled = next_value;
        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }

    pub(crate) fn save_confirm_mode(
        &mut self,
        confirm_mode: ConfirmMode,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let previous_mode = Settings::read(cx, |s| s.confirm.mode);
        if confirm_mode == previous_mode {
            return;
        }

        if let Err(error_message) = Self::persist_settings_update(cx, |settings| {
            settings.confirm.mode = confirm_mode;
        }) {
            Self::notify_settings_save_failed(window, cx, &error_message);
            return;
        }

        self.set_confirm_mode(confirm_mode, window);
        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }

    pub(crate) fn save_hover_preview_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let previous_value = Settings::read(cx, |s| s.preview.hover_preview_enabled);
        if enabled == previous_value {
            return;
        }

        if let Err(error_message) = Self::persist_settings_update(cx, |settings| {
            settings.preview.hover_preview_enabled = enabled;
        }) {
            Self::notify_settings_save_failed(window, cx, &error_message);
            return;
        }

        self.settings_editor.panel_state.hover_preview_enabled = enabled;
        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }

    pub(super) fn sync_autostart_state_for(
        enabled: bool,
    ) -> Result<(), crate::config::AutoStartError> {
        use crate::constants::APP_NAME;
        let manager = crate::config::AutoStartManager::new(APP_NAME)?;
        manager.sync_state(enabled)?;
        Ok(())
    }

    fn is_valid_hotkey_input(input: &str) -> bool {
        let trimmed = input.trim();
        !trimmed.is_empty() && global_hotkey::hotkey::HotKey::from_str(trimmed).is_ok()
    }

    fn has_pending_numeric_input(
        input: &str,
        current_value: usize,
        parser: impl Fn(&str, usize) -> Result<usize, ()>,
    ) -> bool {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return false;
        }

        match parser(trimmed, current_value) {
            Ok(value) => value != current_value,
            Err(()) => true,
        }
    }

    const fn is_valid_storage_pair(max_history: usize, max_storage: usize) -> bool {
        max_storage >= max_history
    }

    /// Validate max history input from settings UI.
    /// Returns `Ok(parsed_value)` when input is valid, or `Err(())` when invalid.
    pub(super) fn parse_max_history_input(input: &str, current_max: usize) -> Result<usize, ()> {
        const MIN: usize = 1;
        const MAX: usize = 10_000;

        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(current_max);
        }

        match trimmed.parse::<usize>() {
            Ok(v) if (MIN..=MAX).contains(&v) => Ok(v),
            _ => Err(()),
        }
    }

    /// Validate max storage input from settings UI.
    /// Returns `Ok(parsed_value)` when input is valid, or `Err(())` when invalid.
    pub(super) fn parse_max_storage_input(input: &str, current_max: usize) -> Result<usize, ()> {
        const MIN: usize = 1;
        const MAX: usize = 100_000;

        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(current_max);
        }

        match trimmed.parse::<usize>() {
            Ok(v) if (MIN..=MAX).contains(&v) => Ok(v),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_max_history_input_empty_uses_current() {
        let current = 42usize;
        let res = RopyBoard::parse_max_history_input("", current);
        assert_eq!(res, Ok(current));
    }

    #[test]
    fn test_parse_max_history_input_valid() {
        let current = 10usize;
        let res = RopyBoard::parse_max_history_input("100", current);
        assert_eq!(res, Ok(100usize));
    }

    #[test]
    fn test_parse_max_history_input_invalid_string() {
        let current = 10usize;
        let res = RopyBoard::parse_max_history_input("abc", current);
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_max_history_input_out_of_range() {
        let current = 10usize;
        // zero is below minimum
        assert!(RopyBoard::parse_max_history_input("0", current).is_err());
        // above maximum (10_000)
        assert!(RopyBoard::parse_max_history_input("10001", current).is_err());
    }

    #[test]
    fn test_is_valid_hotkey_input_empty_returns_false() {
        assert!(!RopyBoard::is_valid_hotkey_input(""));
    }

    #[test]
    fn test_is_valid_hotkey_input_invalid_format_returns_false() {
        assert!(!RopyBoard::is_valid_hotkey_input("not+a+valid+hotkey"));
    }

    #[test]
    fn test_is_valid_hotkey_input_valid_format_returns_true() {
        assert!(RopyBoard::is_valid_hotkey_input("control+shift+d"));
    }

    #[test]
    fn test_resolve_hotkey_candidate_input_empty_uses_current_when_no_pending_change() {
        let resolved = RopyBoard::resolve_hotkey_candidate_input("", "cmd+shift+c", "cmd+shift+c");

        assert_eq!(resolved, "cmd+shift+c");
    }

    #[test]
    fn test_resolve_hotkey_candidate_input_typed_value_takes_precedence() {
        let resolved =
            RopyBoard::resolve_hotkey_candidate_input("cmd+shift+v", "cmd+shift+c", "cmd+shift+c");

        assert_eq!(resolved, "cmd+shift+v");
    }

    #[test]
    fn test_resolve_hotkey_candidate_input_empty_keeps_pending_clear_candidate() {
        let resolved = RopyBoard::resolve_hotkey_candidate_input("", "", "cmd+shift+c");

        assert_eq!(resolved, "");
    }

    #[test]
    fn test_resolve_hotkey_candidate_input_empty_keeps_recorded_candidate() {
        let resolved = RopyBoard::resolve_hotkey_candidate_input("", "cmd+shift+v", "cmd+shift+c");

        assert_eq!(resolved, "cmd+shift+v");
    }

    #[test]
    fn test_normalize_hotkey_for_save_empty_uses_current_hotkey() {
        let normalized = RopyBoard::normalize_hotkey_for_save("", "cmd+shift+c");

        assert_eq!(normalized, "cmd+shift+c");
    }

    #[test]
    fn test_normalize_hotkey_for_save_empty_and_current_empty_uses_default() {
        let normalized = RopyBoard::normalize_hotkey_for_save("", "");

        assert_eq!(normalized, Settings::default().hotkey.activation_key);
    }

    #[test]
    fn test_normalize_hotkey_for_save_explicit_value_preserves_candidate() {
        let normalized = RopyBoard::normalize_hotkey_for_save("cmd+shift+v", "cmd+shift+c");

        assert_eq!(normalized, "cmd+shift+v");
    }

    #[test]
    fn test_is_valid_storage_pair_storage_below_history_returns_false() {
        assert!(!RopyBoard::is_valid_storage_pair(10, 9));
    }

    #[test]
    fn test_is_valid_storage_pair_storage_equal_or_above_history_returns_true() {
        assert!(RopyBoard::is_valid_storage_pair(10, 10));
        assert!(RopyBoard::is_valid_storage_pair(10, 11));
    }

    #[test]
    fn test_has_pending_numeric_input_empty_returns_false() {
        assert!(!RopyBoard::has_pending_numeric_input(
            "",
            10,
            RopyBoard::parse_max_history_input,
        ));
    }

    #[test]
    fn test_has_pending_numeric_input_same_value_returns_false() {
        assert!(!RopyBoard::has_pending_numeric_input(
            "10",
            10,
            RopyBoard::parse_max_history_input,
        ));
    }

    #[test]
    fn test_has_pending_numeric_input_different_value_returns_true() {
        assert!(RopyBoard::has_pending_numeric_input(
            "11",
            10,
            RopyBoard::parse_max_history_input,
        ));
    }

    #[test]
    fn test_has_pending_numeric_input_invalid_value_returns_true() {
        assert!(RopyBoard::has_pending_numeric_input(
            "abc",
            10,
            RopyBoard::parse_max_history_input,
        ));
    }
}

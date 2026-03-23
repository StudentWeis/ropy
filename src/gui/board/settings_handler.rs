use std::str::FromStr;

use gpui::{BorrowAppContext, Context, Window, prelude::Styled, px};
use gpui_component::{WindowExt, notification::Notification};

use super::RopyBoard;
use crate::{config::Settings, i18n::Language};

impl RopyBoard {
    pub(crate) fn resolve_activation_key_input(&self, cx: &Context<Self>) -> String {
        if self.hotkey_manual_editing {
            self.settings_activation_key_input
                .read(cx)
                .value()
                .trim()
                .to_string()
        } else {
            self.pending_hotkey.trim().to_string()
        }
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn save_settings(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        self.hotkey_recording = false;
        let mut activation_key = self.resolve_activation_key_input(cx);

        let mut is_hotkey_invalid = false;
        if activation_key.is_empty() {
            let current_key = Settings::read(cx, |s| s.hotkey.activation_key.clone());
            activation_key = if current_key.is_empty() {
                Settings::default().hotkey.activation_key
            } else {
                current_key
            };
        } else if global_hotkey::hotkey::HotKey::from_str(&activation_key).is_err() {
            is_hotkey_invalid = true;
            activation_key = Settings::default().hotkey.activation_key;
        }

        self.pending_hotkey.clone_from(&activation_key);
        self.hotkey_before_recording.clone_from(&activation_key);
        self.hotkey_manual_editing = false;

        // Get current values from GPUI Global settings as fallback
        let (current_max_history, current_max_storage) = Settings::read(cx, |s| {
            (s.storage.max_history_records, s.storage.max_storage_records)
        });

        // Validate max_history input from the settings UI.
        let max_history_input = self.settings_max_history_input.read(cx).value().to_string();

        let (max_history, is_max_history_invalid) =
            match Self::parse_max_history_input(&max_history_input, current_max_history) {
                Ok(v) => (v, false),
                Err(()) => (current_max_history, true),
            };

        // Validate max_storage input from the settings UI.
        let max_storage_input = self.settings_max_storage_input.read(cx).value().to_string();

        let (mut max_storage, is_max_storage_invalid) =
            match Self::parse_max_storage_input(&max_storage_input, current_max_storage) {
                Ok(v) => (v, false),
                Err(()) => (current_max_storage, true),
            };

        // Ensure max_storage >= max_history
        let is_max_storage_lt_history = max_storage < max_history;
        if is_max_storage_lt_history {
            max_storage = max_history;
        }

        let theme = match self.selected_theme {
            0 => crate::config::AppTheme::Light,
            1 => crate::config::AppTheme::Dark,
            _ => crate::config::AppTheme::System,
        };

        let language = Language::all()
            .get(self.selected_language)
            .cloned()
            .unwrap_or_default();

        // Update GPUI Global settings (auto-persists to disk)
        let autostart_enabled = self.autostart_enabled;
        let auto_check_enabled = self.auto_check_enabled;
        let hover_preview_enabled = self.hover_preview_enabled;
        let confirm_mode = self.confirm_mode;
        let save_disk_error: Option<String> = {
            let activation_key_ref = activation_key.clone();
            let theme_ref = theme.clone();
            let language_ref = language.clone();
            let mut disk_error = None;
            cx.update_global::<Settings, _>(|s, _cx| {
                s.hotkey.activation_key.clone_from(&activation_key_ref);
                s.storage.max_history_records = max_history;
                s.storage.max_storage_records = max_storage;
                s.theme = theme_ref;
                s.autostart.enabled = autostart_enabled;
                s.language = language_ref;
                s.update.auto_check = auto_check_enabled;
                s.preview.hover_preview_enabled = hover_preview_enabled;
                s.confirm.mode = confirm_mode;
                if let Err(e) = s.save() {
                    tracing::warn!(error = %e, "failed to save settings");
                    disk_error = Some(format!("{e}"));
                }
            });
            disk_error
        };

        // Update hotkey if sender is available
        if let Some(tx) = &self.hotkey_tx {
            let _ = tx.try_send(activation_key.clone());
        }

        // Apply the new language
        if let Err(e) = self.i18n.set_language(language) {
            tracing::warn!(error = ?e, "failed to set language");
        }

        // Update tray menu with new language
        self.update_tray_menu();

        // Update search placeholder with new language
        let search_placeholder = self.i18n.t("search_placeholder");
        self.search_input.update(cx, |input, cx| {
            input.set_placeholder(search_placeholder, window, cx);
        });

        // Sync auto-start state with system
        let autostart_error = self.sync_autostart_state().err();
        if let Some(ref e) = autostart_error {
            tracing::warn!(error = ?e, "failed to sync auto-start state");
        }

        // Apply the new theme
        let app_theme = &theme.get_theme();
        crate::gui::app::set_app_theme(window, cx, app_theme);

        self.settings_max_history_input.update(cx, |input, cx| {
            input.set_placeholder(max_history.to_string(), window, cx);
            input.set_value("", window, cx);
        });

        self.settings_max_storage_input.update(cx, |input, cx| {
            input.set_placeholder(max_storage.to_string(), window, cx);
            input.set_value("", window, cx);
        });

        // --- User notifications: auto width (content-driven), capped at 280px ---
        if let Some(err_msg) = save_disk_error {
            let msg = format!("✕  {}: {}", self.i18n.t("settings_save_failed"), err_msg);
            window.push_notification(
                Notification::new().message(msg).w_auto().max_w(px(280.0)),
                cx,
            );
        } else {
            if is_hotkey_invalid {
                let warn_msg = self.i18n.t("settings_hotkey_invalid_warning");
                window.push_notification(
                    Notification::new()
                        .message(format!("⚠  {warn_msg}"))
                        .w_auto()
                        .max_w(px(280.0)),
                    cx,
                );
            }
            if is_max_history_invalid {
                let warn_msg = self.i18n.t("settings_max_history_invalid_warning");
                window.push_notification(
                    Notification::new()
                        .message(format!("⚠  {warn_msg}"))
                        .w_auto()
                        .max_w(px(280.0)),
                    cx,
                );
            }
            if is_max_storage_invalid {
                let warn_msg = self.i18n.t("settings_max_storage_invalid_warning");
                window.push_notification(
                    Notification::new()
                        .message(format!("⚠  {warn_msg}"))
                        .w_auto()
                        .max_w(px(280.0)),
                    cx,
                );
            }
            if is_max_storage_lt_history {
                let warn_msg = self.i18n.t("settings_max_storage_lt_history_warning");
                window.push_notification(
                    Notification::new()
                        .message(format!("⚠  {warn_msg}"))
                        .w_auto()
                        .max_w(px(280.0)),
                    cx,
                );
            }
            if autostart_error.is_some() {
                let warn_msg = self.i18n.t("settings_autostart_failed");
                window.push_notification(
                    Notification::new()
                        .message(format!("⚠  {warn_msg}"))
                        .w_auto()
                        .max_w(px(280.0)),
                    cx,
                );
            }
            if !is_hotkey_invalid
                && autostart_error.is_none()
                && !is_max_history_invalid
                && !is_max_storage_invalid
                && !is_max_storage_lt_history
            {
                let ok_msg = self.i18n.t("settings_save_success");
                window.push_notification(
                    Notification::new()
                        .message(format!("✓  {ok_msg}"))
                        .w_auto()
                        .max_w(px(280.0)),
                    cx,
                );
            }
        }

        cx.notify();
    }

    pub(crate) fn toggle_autostart(&mut self, cx: &mut Context<Self>) {
        self.autostart_enabled = !self.autostart_enabled;
        cx.notify();
    }

    pub(super) fn sync_autostart_state(&self) -> Result<(), crate::config::AutoStartError> {
        use crate::constants::APP_NAME;
        let manager = crate::config::AutoStartManager::new(APP_NAME)?;
        manager.sync_state(self.autostart_enabled)?;
        Ok(())
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
}

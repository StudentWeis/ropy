use std::str::FromStr;

use gpui::{BorrowAppContext, Context, Window, prelude::Styled, px};
use gpui_component::{WindowExt, notification::Notification};

use super::{RopyBoard, settings_editor};
use crate::{
    config::{ConfirmMode, LayoutMode, Settings},
    gui::theme::ThemeId,
    i18n::{I18n, Language},
    repository::GlobalRepository,
};

const SETTINGS_NOTIFICATION_MAX_WIDTH_PX: f32 = 280.0;

impl RopyBoard {
    pub(crate) fn hotkey_placeholder_text(hotkey: &str, cx: &Context<Self>) -> gpui::SharedString {
        if hotkey.trim().is_empty() {
            I18n::translate(cx, "settings_hotkey_empty").into()
        } else {
            hotkey.trim().to_owned().into()
        }
    }

    fn resolve_hotkey_candidate_input(
        input: &str,
        pending_hotkey: &str,
        current_hotkey: &str,
    ) -> String {
        let trimmed_input = input.trim();
        if !trimmed_input.is_empty() {
            return trimmed_input.to_string();
        }

        if pending_hotkey.trim() != current_hotkey.trim() {
            return pending_hotkey.trim().to_string();
        }

        current_hotkey.trim().to_string()
    }

    fn normalize_hotkey_for_save(candidate_hotkey: &str, current_hotkey: &str) -> String {
        let trimmed_candidate = candidate_hotkey.trim();
        if !trimmed_candidate.is_empty() {
            return trimmed_candidate.to_string();
        }

        let trimmed_current = current_hotkey.trim();
        if !trimmed_current.is_empty() {
            return trimmed_current.to_string();
        }

        Settings::default().hotkey.activation_key
    }

    pub(crate) fn refresh_activation_key_placeholder(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current_hotkey = Settings::read(cx, |s| s.hotkey.activation_key.clone());
        let placeholder = Self::hotkey_placeholder_text(&current_hotkey, cx);

        self.settings_editor
            .settings_activation_key_input
            .update(cx, |input, cx| {
                input.set_placeholder(placeholder.clone(), window, cx);
            });
    }

    pub(crate) fn sync_activation_key_input(
        &self,
        value: &str,
        placeholder_hotkey: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let next_value = value.to_string();
        let placeholder = Self::hotkey_placeholder_text(placeholder_hotkey, cx);

        self.settings_editor
            .settings_activation_key_input
            .update(cx, |input, cx| {
                input.set_placeholder(placeholder.clone(), window, cx);
                input.set_value(next_value.clone(), window, cx);
            });
    }

    pub(crate) fn sync_activation_key_input_from_candidate(
        &self,
        candidate: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let current_hotkey = Settings::read(cx, |s| s.hotkey.activation_key.clone());
        let value = if candidate.trim() == current_hotkey.trim() {
            ""
        } else {
            candidate.trim()
        };

        self.sync_activation_key_input(value, &current_hotkey, window, cx);
    }

    pub(crate) fn resolve_activation_key_input(&self, cx: &Context<Self>) -> String {
        let current_hotkey = Settings::read(cx, |s| s.hotkey.activation_key.clone());
        let input_value = self
            .settings_editor
            .settings_activation_key_input
            .read(cx)
            .value()
            .to_string();

        Self::resolve_hotkey_candidate_input(
            &input_value,
            &self.settings_editor.pending_hotkey,
            &current_hotkey,
        )
    }

    pub(crate) fn has_pending_hotkey(&self, cx: &Context<Self>) -> bool {
        let current_hotkey = Settings::read(cx, |s| s.hotkey.activation_key.clone());
        let candidate_hotkey = self.resolve_activation_key_input(cx);

        Self::normalize_hotkey_for_save(&candidate_hotkey, &current_hotkey) != current_hotkey
    }

    pub(crate) fn has_pending_max_history(&self, cx: &Context<Self>) -> bool {
        let current_max_history = Settings::read(cx, |s| s.storage.max_history_records);
        let input = self
            .settings_editor
            .settings_max_history_input
            .read(cx)
            .value()
            .to_string();
        Self::has_pending_numeric_input(&input, current_max_history, Self::parse_max_history_input)
    }

    pub(crate) fn has_pending_max_storage(&self, cx: &Context<Self>) -> bool {
        let current_max_storage = Settings::read(cx, |s| s.storage.max_storage_records);
        let input = self
            .settings_editor
            .settings_max_storage_input
            .read(cx)
            .value()
            .to_string();
        Self::has_pending_numeric_input(&input, current_max_storage, Self::parse_max_storage_input)
    }

    fn push_settings_notification(
        window: &mut Window,
        notification: Notification,
        cx: &mut Context<Self>,
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
        cx: &mut Context<Self>,
        message: impl Into<gpui::SharedString>,
    ) {
        Self::push_settings_notification(window, Notification::warning(message.into()), cx);
    }

    fn notify_settings_success(
        window: &mut Window,
        cx: &mut Context<Self>,
        message: impl Into<gpui::SharedString>,
    ) {
        Self::push_settings_notification(window, Notification::success(message.into()), cx);
    }

    fn notify_settings_save_failed(
        window: &mut Window,
        cx: &mut Context<Self>,
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
        cx: &mut Context<Self>,
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
        cx: &mut Context<Self>,
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
        cx: &mut Context<Self>,
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
        cx: &mut Context<Self>,
    ) {
        self.settings_editor.selected_layout = layout_idx;
        self.sync_layout_select_items(window, cx);
    }

    pub(crate) fn sync_layout_select_items(&self, window: &mut Window, cx: &mut Context<Self>) {
        settings_editor::sync_layout_select_items(
            &self.settings_editor.layout_select,
            self.settings_editor.selected_layout,
            window,
            cx,
        );
    }

    pub(crate) fn save_selected_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previous_layout = Settings::read(cx, |s| s.layout.mode);
        let next_layout = LayoutMode::all()
            .get(self.settings_editor.selected_layout)
            .copied()
            .unwrap_or_default();

        if next_layout == previous_layout {
            return;
        }

        if let Err(error_message) = Self::persist_settings_update(cx, |settings| {
            settings.layout.mode = next_layout;
        }) {
            let previous_layout_idx = LayoutMode::all()
                .iter()
                .position(|mode| mode == &previous_layout)
                .unwrap_or_default();
            self.set_layout_selection(previous_layout_idx, window, cx);
            Self::notify_settings_save_failed(window, cx, &error_message);
            cx.notify();
            return;
        }

        self.layout_mode = next_layout;
        self.list_state
            .reset(self.visible_list_len(self.filtered_record_indices.len()));
        self.force_reveal_selected_record();
        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }

    pub(crate) fn save_selected_theme(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previous_theme = Settings::read(cx, |s| s.theme.clone());
        let next_theme = ThemeId::all()
            .get(self.settings_editor.selected_theme)
            .cloned()
            .unwrap_or_default();

        if next_theme == previous_theme {
            return;
        }

        let theme_to_save = next_theme.clone();
        if let Err(error_message) = Self::persist_settings_update(cx, move |settings| {
            settings.theme = theme_to_save;
        }) {
            let previous_theme_idx = ThemeId::all()
                .iter()
                .position(|theme_id| theme_id == &previous_theme)
                .unwrap_or_default();
            self.set_theme_selection(previous_theme_idx, window, cx);
            crate::gui::app::set_app_theme(
                window,
                cx,
                &previous_theme,
                self.settings_editor.window_opacity_percent,
            );
            crate::gui::app::apply_window_opacity(
                window,
                self.settings_editor.window_opacity_percent,
            );
            Self::notify_settings_save_failed(window, cx, &error_message);
            cx.notify();
            return;
        }

        crate::gui::app::set_app_theme(
            window,
            cx,
            &next_theme,
            self.settings_editor.window_opacity_percent,
        );
        crate::gui::app::apply_window_opacity(window, self.settings_editor.window_opacity_percent);
        cx.notify();
    }

    pub(crate) fn save_selected_language(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previous_language = Settings::read(cx, |s| s.language.clone());
        let next_language = Language::all()
            .get(self.settings_editor.selected_language)
            .cloned()
            .unwrap_or_default();

        if next_language == previous_language {
            return;
        }

        let language_to_save = next_language.clone();
        if let Err(error_message) = Self::persist_settings_update(cx, move |settings| {
            settings.language = language_to_save;
        }) {
            let previous_language_idx = Language::all()
                .iter()
                .position(|language| language == &previous_language)
                .unwrap_or_default();
            self.set_language_selection(previous_language_idx, window, cx);
            Self::notify_settings_save_failed(window, cx, &error_message);
            cx.notify();
            return;
        }

        cx.update_global::<I18n, _>(|i18n: &mut I18n, _cx| {
            if let Err(error) = i18n.set_language(next_language) {
                tracing::warn!(error = ?error, "failed to set language");
            }
        });
        Self::update_tray_menu(cx);
        self.refresh_activation_key_placeholder(window, cx);
        self.sync_layout_select_items(window, cx);
        cx.notify();
    }

    pub(crate) fn save_window_opacity(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previous_opacity = Settings::read(cx, |s| s.window.opacity_percent);
        let next_opacity = self.settings_editor.window_opacity_percent;

        if next_opacity == previous_opacity {
            return;
        }

        if let Err(error_message) = Self::persist_settings_update(cx, |settings| {
            settings.window.opacity_percent = next_opacity;
            settings.window.normalize_opacity();
        }) {
            self.settings_editor.window_opacity_percent = previous_opacity;
            self.sync_window_opacity_slider(previous_opacity, window, cx);
            let theme = ThemeId::all()
                .get(self.settings_editor.selected_theme)
                .cloned()
                .unwrap_or_default();
            crate::gui::app::set_app_theme(window, cx, &theme, previous_opacity);
            crate::gui::app::apply_window_opacity(window, previous_opacity);
            Self::notify_settings_save_failed(window, cx, &error_message);
            cx.notify();
            return;
        }

        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }

    pub(crate) fn save_hotkey(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        let current_hotkey = Settings::read(cx, |s| s.hotkey.activation_key.clone());
        let activation_key = Self::normalize_hotkey_for_save(
            &self.resolve_activation_key_input(cx),
            &current_hotkey,
        );

        if !activation_key.is_empty() && !Self::is_valid_hotkey_input(&activation_key) {
            Self::notify_settings_warning(
                window,
                cx,
                I18n::translate(cx, "settings_hotkey_invalid_warning"),
            );
            return;
        }

        if activation_key == current_hotkey {
            self.settings_editor.hotkey_recording = false;
            self.settings_editor
                .pending_hotkey
                .clone_from(&activation_key);
            self.settings_editor
                .hotkey_before_recording
                .clone_from(&activation_key);
            self.sync_activation_key_input("", &activation_key, window, cx);
            cx.notify();
            return;
        }

        let activation_key_to_save = activation_key.clone();
        if let Err(error_message) = Self::persist_settings_update(cx, move |settings| {
            settings.hotkey.activation_key = activation_key_to_save;
        }) {
            Self::notify_settings_save_failed(window, cx, &error_message);
            return;
        }

        self.settings_editor.hotkey_recording = false;
        self.settings_editor
            .pending_hotkey
            .clone_from(&activation_key);
        self.settings_editor
            .hotkey_before_recording
            .clone_from(&activation_key);
        self.sync_activation_key_input("", &activation_key, window, cx);

        if let Some(tx) = &self.hotkey_tx {
            let _ = tx.try_send(activation_key);
        }

        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }

    pub(crate) fn save_max_history(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        let (current_max_history, current_max_storage) = Settings::read(cx, |s| {
            (s.storage.max_history_records, s.storage.max_storage_records)
        });
        let max_history_input = self
            .settings_editor
            .settings_max_history_input
            .read(cx)
            .value()
            .to_string();
        let Ok(max_history) =
            Self::parse_max_history_input(&max_history_input, current_max_history)
        else {
            self.settings_editor
                .settings_max_history_input
                .update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
            Self::notify_settings_warning(
                window,
                cx,
                I18n::translate(cx, "settings_max_history_invalid_warning"),
            );
            return;
        };

        if !Self::is_valid_storage_pair(max_history, current_max_storage) {
            self.settings_editor
                .settings_max_history_input
                .update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
            Self::notify_settings_warning(
                window,
                cx,
                I18n::translate(cx, "settings_max_storage_lt_history_warning"),
            );
            return;
        }

        if max_history == current_max_history {
            self.settings_editor
                .settings_max_history_input
                .update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
            cx.notify();
            return;
        }

        if let Err(error_message) = Self::persist_settings_update(cx, |settings| {
            settings.storage.max_history_records = max_history;
        }) {
            Self::notify_settings_save_failed(window, cx, &error_message);
            return;
        }

        self.settings_editor
            .settings_max_history_input
            .update(cx, |input, cx| {
                input.set_placeholder(max_history.to_string(), window, cx);
                input.set_value("", window, cx);
            });
        self.refresh_records_from_repository(cx);
        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }

    pub(crate) fn save_max_storage(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        let (current_max_history, current_max_storage) = Settings::read(cx, |s| {
            (s.storage.max_history_records, s.storage.max_storage_records)
        });
        let max_storage_input = self
            .settings_editor
            .settings_max_storage_input
            .read(cx)
            .value()
            .to_string();
        let Ok(max_storage) =
            Self::parse_max_storage_input(&max_storage_input, current_max_storage)
        else {
            self.settings_editor
                .settings_max_storage_input
                .update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
            Self::notify_settings_warning(
                window,
                cx,
                I18n::translate(cx, "settings_max_storage_invalid_warning"),
            );
            return;
        };

        if !Self::is_valid_storage_pair(current_max_history, max_storage) {
            self.settings_editor
                .settings_max_storage_input
                .update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
            Self::notify_settings_warning(
                window,
                cx,
                I18n::translate(cx, "settings_max_storage_lt_history_warning"),
            );
            return;
        }

        if max_storage == current_max_storage {
            self.settings_editor
                .settings_max_storage_input
                .update(cx, |input, cx| {
                    input.set_value("", window, cx);
                });
            cx.notify();
            return;
        }

        if let Err(error_message) = Self::persist_settings_update(cx, |settings| {
            settings.storage.max_storage_records = max_storage;
        }) {
            Self::notify_settings_save_failed(window, cx, &error_message);
            return;
        }

        self.settings_editor
            .settings_max_storage_input
            .update(cx, |input, cx| {
                input.set_placeholder(max_storage.to_string(), window, cx);
                input.set_value("", window, cx);
            });

        GlobalRepository::read(cx, |repo| {
            let Some(repo) = repo else {
                return;
            };

            if let Err(error) = repo.cleanup_old_records(max_storage) {
                tracing::warn!(error = %error, "failed to apply storage limit after saving settings");
            }
        });
        self.refresh_records_from_repository(cx);
        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }

    pub(crate) fn toggle_autostart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previous_value = Settings::read(cx, |s| s.autostart.enabled);
        let next_value = !self.settings_editor.autostart_enabled;

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
            self.settings_editor.autostart_enabled = Settings::read(cx, |s| s.autostart.enabled);
            Self::notify_settings_warning(
                window,
                cx,
                I18n::translate(cx, "settings_autostart_failed"),
            );
            cx.notify();
            return;
        }

        self.settings_editor.autostart_enabled = next_value;
        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }

    pub(crate) fn save_confirm_mode(
        &mut self,
        confirm_mode: ConfirmMode,
        window: &mut Window,
        cx: &mut Context<Self>,
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

    pub(crate) fn save_auto_check_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
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

        self.settings_editor.auto_check_enabled = enabled;
        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }

    pub(crate) fn save_include_prerelease_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
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

        self.settings_editor.include_prerelease_enabled = enabled;
        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }

    pub(crate) fn save_hover_preview_enabled(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
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

        self.settings_editor.hover_preview_enabled = enabled;
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

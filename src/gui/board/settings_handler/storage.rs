//! Storage limit handlers (`max_history_records`, `max_storage_records`).

use gpui::{Context, Window};

use super::RopyBoard;
use crate::{config::Settings, i18n::I18n, repository::GlobalRepository};

impl RopyBoard {
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
}

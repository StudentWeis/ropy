//! Hotkey-related settings handlers for [`RopyBoard`].
//!
//! Owns the "activation key" recording/validation flow and the
//! `save_hotkey` path. Shared notification and persistence helpers live
//! in the parent module.

use gpui_kit::{Context, Window};

use super::RopyBoard;
use crate::{
    config::Settings,
    gui::hotkey::{HotkeyUpdateError, request_hotkey_update},
    i18n::I18n,
};

impl RopyBoard {
    pub(crate) fn hotkey_placeholder_text(
        hotkey: &str,
        cx: &Context<'_, Self>,
    ) -> gpui_kit::SharedString {
        if hotkey.trim().is_empty() {
            I18n::translate(cx, "settings_hotkey_empty").into()
        } else {
            hotkey.trim().to_owned().into()
        }
    }

    pub(super) fn resolve_hotkey_candidate_input(
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

    pub(super) fn normalize_hotkey_for_save(
        candidate_hotkey: &str,
        current_hotkey: &str,
    ) -> String {
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
        cx: &mut Context<'_, Self>,
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
        cx: &mut Context<'_, Self>,
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
        cx: &mut Context<'_, Self>,
    ) {
        let current_hotkey = Settings::read(cx, |s| s.hotkey.activation_key.clone());
        let value = if candidate.trim() == current_hotkey.trim() {
            ""
        } else {
            candidate.trim()
        };

        self.sync_activation_key_input(value, &current_hotkey, window, cx);
    }

    pub(crate) fn resolve_activation_key_input(&self, cx: &Context<'_, Self>) -> String {
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

    pub(crate) fn has_pending_hotkey(&self, cx: &Context<'_, Self>) -> bool {
        let current_hotkey = Settings::read(cx, |s| s.hotkey.activation_key.clone());
        let candidate_hotkey = self.resolve_activation_key_input(cx);

        Self::normalize_hotkey_for_save(&candidate_hotkey, &current_hotkey) != current_hotkey
    }

    pub(crate) fn save_hotkey(&mut self, cx: &mut Context<'_, Self>, window: &mut Window) {
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

        if self.settings_editor.hotkey.saving {
            return;
        }
        let response = self
            .hotkey_tx
            .as_ref()
            .ok_or(HotkeyUpdateError::Disconnected)
            .and_then(|tx| request_hotkey_update(tx, activation_key.clone()));
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                Self::notify_hotkey_update_failed(&error, window, cx);
                return;
            }
        };
        self.settings_editor.hotkey.saving = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let result = response
                .recv()
                .await
                .unwrap_or(Err(HotkeyUpdateError::Disconnected));
            let _ = this.update_in(cx, |board, window, cx| {
                board.settings_editor.hotkey.saving = false;
                match result {
                    Ok(()) => {
                        board.settings_editor.hotkey.recording = false;
                        board
                            .settings_editor
                            .pending_hotkey
                            .clone_from(&activation_key);
                        board
                            .settings_editor
                            .hotkey_before_recording
                            .clone_from(&activation_key);
                        board.sync_activation_key_input("", &activation_key, window, cx);
                        Self::notify_settings_success(
                            window,
                            cx,
                            I18n::translate(cx, "settings_save_success"),
                        );
                    }
                    Err(error) => Self::notify_hotkey_update_failed(&error, window, cx),
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn notify_hotkey_update_failed(
        error: &HotkeyUpdateError,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        tracing::warn!(%error, "hotkey update failed");
        Self::notify_settings_warning(
            window,
            cx,
            I18n::translate(cx, "settings_hotkey_update_failed"),
        );
    }
}

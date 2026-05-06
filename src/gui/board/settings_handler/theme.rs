//! Visual / locale settings handlers: theme, language, and window opacity.

use gpui::{BorrowAppContext, Context, Window};

use super::RopyBoard;
use crate::{
    config::Settings,
    gui::theme::ThemeId,
    i18n::{I18n, Language},
};

impl RopyBoard {
    pub(crate) fn save_selected_theme(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
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

    pub(crate) fn save_selected_language(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
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

    pub(crate) fn save_window_opacity(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
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
}

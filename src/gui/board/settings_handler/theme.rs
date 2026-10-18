//! Visual / locale settings handlers: theme, language, and window opacity.

use gpui_kit::{Context, Window};
use rust_i18n::t;

use super::RopyBoard;
use crate::gui::settings::GlobalSettings;

impl RopyBoard {
    pub(crate) fn save_selected_theme(
        &self,
        target: super::settings_editor::theme::ThemeTarget,
        id: &crate::gui::theme::ThemeId,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let previous = GlobalSettings::read(cx, |settings| {
            crate::gui::theme::ThemeCatalog::global(cx)
                .resolve(settings, window.appearance().into())
        });
        let id = id.clone();
        if let Err(message) = Self::persist_settings_update(cx, |settings| target.set(settings, id))
        {
            self.settings_editor.theme.sync(window, cx);
            Self::notify_settings_save_failed(window, cx, &message);
            cx.notify();
            return;
        }
        let next = GlobalSettings::read(cx, |settings| {
            crate::gui::theme::ThemeCatalog::global(cx)
                .resolve(settings, window.appearance().into())
        });
        if next != previous {
            crate::gui::app::apply_current_theme(
                window,
                cx,
                self.settings_editor.window_opacity_percent,
            );
        }
        cx.notify();
    }

    pub(crate) fn save_follow_system(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if GlobalSettings::read(cx, |settings| settings.appearance.follow_system) == enabled {
            return;
        }
        let mode = window.appearance().into();
        let catalog = crate::gui::theme::ThemeCatalog::global(cx).clone();
        if let Err(message) = Self::persist_settings_update(cx, |settings| {
            catalog.set_follow_system(settings, enabled, mode);
        }) {
            Self::notify_settings_save_failed(window, cx, &message);
            cx.notify();
            return;
        }
        self.settings_editor.theme.update_system_mode(mode);
        self.settings_editor.theme.sync(window, cx);
        crate::gui::app::apply_current_theme(
            window,
            cx,
            self.settings_editor.window_opacity_percent,
        );
        cx.notify();
    }

    pub(crate) fn refresh_system_appearance(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if !self
            .settings_editor
            .theme
            .update_system_mode(window.appearance().into())
        {
            return;
        }
        if GlobalSettings::read(cx, |settings| settings.appearance.follow_system) {
            crate::gui::app::apply_current_theme(
                window,
                cx,
                self.settings_editor.window_opacity_percent,
            );
        }
        cx.notify();
    }

    pub(crate) fn save_selected_language(
        &mut self,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let previous_language = GlobalSettings::read(cx, |s| s.language.clone());
        let next_language = crate::gui::settings::available_languages()
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
            let previous_language_idx = crate::gui::settings::available_languages()
                .iter()
                .position(|language| language == &previous_language)
                .unwrap_or_default();
            self.set_language_selection(previous_language_idx, window, cx);
            Self::notify_settings_save_failed(window, cx, &error_message);
            cx.notify();
            return;
        }

        rust_i18n::set_locale(next_language.code());
        Self::update_tray_menu(cx);
        self.refresh_activation_key_placeholder(window, cx);
        self.sync_layout_select_items(window, cx);
        self.settings_editor.theme.sync(window, cx);
        cx.notify();
    }

    pub(crate) fn save_window_opacity(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let previous_opacity = GlobalSettings::read(cx, |s| s.window.opacity_percent);
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
            crate::gui::app::apply_current_theme(window, cx, previous_opacity);
            crate::gui::app::apply_window_opacity(window, previous_opacity);
            Self::notify_settings_save_failed(window, cx, &error_message);
            cx.notify();
            return;
        }

        Self::notify_settings_success(window, cx, t!("settings_save_success"));
        cx.notify();
    }
}

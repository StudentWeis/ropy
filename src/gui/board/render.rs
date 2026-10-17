use gpui_kit::{
    AnyElement, Context, Render, Window,
    base::TestSupportExt,
    component::{ActiveTheme, alert::Alert, v_flex},
    prelude::{InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled},
};
use rust_i18n::t;

use super::{ActivePanel, RopyBoard, header::render_header, search::render_search_input};
use crate::{
    config::Settings,
    gui::{
        panel::{
            about::render_about_content, help::render_help_content,
            settings::render_settings_content,
        },
        repository::GlobalRepository,
        settings::GlobalSettings,
    },
};

impl Render for RopyBoard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let mut base = v_flex()
            .id("ropy-board")
            .track_focus(&self.focus_handle)
            .key_context(self.shortcut_context())
            .on_action(cx.listener(Self::on_hide_action))
            .on_action(cx.listener(Self::on_quit_action))
            .on_action(cx.listener(Self::on_active_action))
            .on_key_up(cx.listener(Self::on_key_up))
            .size_full()
            .px_4()
            .pb_4()
            .bg(cx.theme().background);

        if !self.activated {
            return gpui_kit::div().size_full().child(base).into_any_element();
        }

        if GlobalSettings::read(cx, Settings::is_recovery_required) {
            let message = gpui_kit::SharedString::from(t!("settings_recovery_required"));
            base = base.child(
                gpui_kit::div()
                    .id("settings-recovery")
                    .test_support()
                    .aria_label(message.clone())
                    .py_2()
                    .child(Alert::error("settings-recovery-alert", message)),
            );
        }
        if GlobalRepository::read(cx, |repository| repository.is_none()) {
            let message = gpui_kit::SharedString::from(t!("repository_unavailable"));
            base = base.child(
                gpui_kit::div()
                    .id("repository-unavailable")
                    .test_support()
                    .aria_label(message.clone())
                    .py_2()
                    .child(Alert::error("repository-unavailable-alert", message)),
            );
        }

        if let Some(message) = super::actions::keybinding_error_message(cx) {
            base = base.child(
                gpui_kit::div()
                    .id("keybindings-error")
                    .test_support()
                    .aria_label(message.clone())
                    .py_2()
                    .child(Alert::error("keybindings-error-alert", message)),
            );
        }

        let body: AnyElement = match self.active_panel {
            ActivePanel::Settings => base
                .on_key_down(cx.listener(Self::on_settings_key_down))
                .child(render_settings_content(self, cx))
                .into_any_element(),
            ActivePanel::About => base
                .child(render_about_content(self, cx))
                .into_any_element(),
            ActivePanel::Help => base.child(render_help_content(self, cx)).into_any_element(),
            ActivePanel::ClipboardList => base
                .on_action(cx.listener(Self::on_select_left))
                .on_action(cx.listener(Self::on_select_right))
                .on_action(cx.listener(Self::on_select_prev))
                .on_action(cx.listener(Self::on_select_next))
                .on_action(cx.listener(Self::on_confirm_selection))
                .on_action(cx.listener(Self::on_confirm_selection_plain_text))
                .on_action(cx.listener(Self::on_delete_record))
                .on_action(cx.listener(Self::on_cycle_filter_next))
                .on_action(cx.listener(Self::on_cycle_filter_prev))
                .on_action(cx.listener(Self::on_toggle_favorites_filter))
                .on_action(cx.listener(Self::on_focus_search))
                .on_action(cx.listener(Self::on_preview_selection))
                .on_action(cx.listener(Self::on_toggle_window_pin))
                .on_action(cx.listener(Self::on_toggle_favorite))
                .on_action(cx.listener(Self::on_confirm_record))
                .child(render_header(self, cx))
                .child(render_search_input(self, cx))
                .child(self.render_records_list(window, cx))
                .into_any_element(),
        };

        body
    }
}

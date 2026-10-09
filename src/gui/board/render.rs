use gpui_kit::{
    AnyElement, Context, Render, Window,
    base::TestSupportExt,
    component::{ActiveTheme, alert::Alert, v_flex},
    prelude::{InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled},
};

use super::{ActivePanel, RopyBoard, header::render_header, search::render_search_input};
use crate::{
    config::Settings,
    gui::panel::{
        about::render_about_content, help::render_help_content, settings::render_settings_content,
    },
    i18n::I18n,
    repository::GlobalRepository,
};

impl Render for RopyBoard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let surface_bg = self.main_panel_surface(cx.theme().background);

        let mut base = v_flex()
            .id("ropy-board")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_hide_action))
            .on_action(cx.listener(Self::on_quit_action))
            .on_action(cx.listener(Self::on_active_action))
            .size_full()
            .px_4()
            .pb_4()
            .bg(surface_bg);

        if !self.activated {
            return gpui_kit::div().size_full().child(base).into_any_element();
        }

        if Settings::read(cx, Settings::is_recovery_required) {
            let message = I18n::translate(cx, "settings_recovery_required");
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
            let message = I18n::translate(cx, "repository_unavailable");
            base = base.child(
                gpui_kit::div()
                    .id("repository-unavailable")
                    .test_support()
                    .aria_label(message.clone())
                    .py_2()
                    .child(Alert::error("repository-unavailable-alert", message)),
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
                .on_key_down(cx.listener(Self::on_key_down))
                .on_key_up(cx.listener(Self::on_key_up))
                .child(render_header(self, cx))
                .child(render_search_input(self, cx))
                .child(self.render_records_list(window, cx))
                .into_any_element(),
        };

        body
    }
}

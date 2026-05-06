use gpui::{
    AnyElement, Context, Render, Window,
    prelude::{FluentBuilder, InteractiveElement, IntoElement, ParentElement, Styled},
};
use gpui_component::{ActiveTheme, WindowExt, v_flex};

use super::{
    ActivePanel, RopyBoard, clear_confirm, header::render_header, search::render_search_input,
};
use crate::gui::panel::{
    about::render_about_content, help::render_help_content, settings::render_settings_content,
};

impl Render for RopyBoard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let base = v_flex()
            .id("ropy-board")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_hide_action))
            .on_action(cx.listener(Self::on_quit_action))
            .on_action(cx.listener(Self::on_active_action))
            .size_full()
            .px_4()
            .pb_4();

        let body: AnyElement = match self.active_panel {
            ActivePanel::Settings => base
                .bg(self.main_panel_surface(cx.theme().background))
                .on_key_down(cx.listener(Self::on_settings_key_down))
                .child(render_settings_content(self, cx))
                .into_any_element(),
            ActivePanel::About => base
                .bg(self.main_panel_surface(cx.theme().background))
                .child(render_about_content(self, cx))
                .into_any_element(),
            ActivePanel::Help => base
                .bg(self.main_panel_surface(cx.theme().background))
                .child(render_help_content(self, cx))
                .into_any_element(),
            ActivePanel::ClipboardList => base
                .bg(self.main_panel_surface(cx.theme().background))
                .on_action(cx.listener(Self::on_select_left))
                .on_action(cx.listener(Self::on_select_right))
                .on_action(cx.listener(Self::on_select_prev))
                .on_action(cx.listener(Self::on_select_next))
                .on_action(cx.listener(Self::on_confirm_selection))
                .on_action(cx.listener(Self::on_confirm_selection_plain_text))
                .on_action(cx.listener(Self::on_delete_record))
                .on_key_down(cx.listener(Self::on_key_down))
                .child(render_header(self, cx))
                .child(render_search_input(self, cx))
                .child(self.render_records_list(window, cx))
                .into_any_element(),
        };

        // Render each notification directly in a bottom-right column.
        let notifs: Vec<_> = window.notifications(cx).iter().cloned().collect();
        let has_notifs = !notifs.is_empty();
        let show_clear_confirm = self.show_clear_confirm;
        let clear_confirm_action = self.clear_confirm_action;
        gpui::div()
            .relative()
            .size_full()
            .child(body)
            .when(show_clear_confirm, |this| {
                this.child(clear_confirm::render_clear_confirm_overlay(
                    clear_confirm_action,
                    cx,
                ))
            })
            .when(has_notifs, move |this| {
                this.child(
                    v_flex()
                        .absolute()
                        .bottom_4()
                        .right_3()
                        .gap_2()
                        .opacity(0.9)
                        .children(notifs),
                )
            })
    }
}

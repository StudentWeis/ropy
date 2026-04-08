use std::sync::Arc;

use gpui::{
    AnyElement, Context, Render, Window,
    prelude::{FluentBuilder, InteractiveElement, IntoElement, ParentElement, Styled},
};
use gpui_component::{ActiveTheme, WindowExt, v_flex};

use super::{RopyBoard, clear_confirm, header::render_header, search::render_search_input};
use crate::gui::panel::{
    about::render_about_content, help::render_help_content, settings::render_settings_content,
};

impl Render for RopyBoard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let base = v_flex()
            .id("ropy-board")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_hide_action))
            .on_action(cx.listener(Self::on_quit_action))
            .on_action(cx.listener(Self::on_active_action))
            .size_full()
            .px_4()
            .pb_4();

        let body: AnyElement = if self.show_settings {
            base.bg(self.main_panel_surface(cx.theme().background))
                .on_key_down(cx.listener(Self::on_settings_key_down))
                .child(render_settings_content(self, cx))
                .into_any_element()
        } else if self.show_about {
            base.bg(self.main_panel_surface(cx.theme().background))
                .child(render_about_content(self, cx))
                .into_any_element()
        } else if self.show_help {
            base.bg(self.main_panel_surface(cx.theme().background))
                .child(render_help_content(self, cx))
                .into_any_element()
        } else {
            // Render main clipboard view
            let query = self.search_input.read(cx).value().to_string();
            let new_filtered_record_indices = self.get_filtered_record_indices(&query);

            if new_filtered_record_indices != *self.filtered_record_indices {
                let old_len = self.filtered_record_indices.len();
                let new_len = new_filtered_record_indices.len();

                // If we're deleting a record, preserve the scroll position
                let scroll_position = if self.deleting_record {
                    Some(self.list_state.logical_scroll_top())
                } else {
                    None
                };

                self.filtered_record_indices = Arc::new(new_filtered_record_indices);

                // Use splice to inform list state about the change instead of reset
                // This helps preserve scroll position better
                if self.deleting_record {
                    self.list_state.splice(0..old_len, new_len);

                    // Restore scroll position
                    if let Some(scroll_pos) = scroll_position {
                        self.list_state.scroll_to(scroll_pos);
                    }

                    // Reset the flag
                    self.deleting_record = false;
                } else {
                    // For other changes (like search), reset the list state
                    self.list_state.reset(new_len);
                }
            }

            if self.selected_index >= self.filtered_record_indices.len()
                && !self.filtered_record_indices.is_empty()
            {
                self.selected_index = self.filtered_record_indices.len() - 1;
            } else if self.filtered_record_indices.is_empty() {
                self.selected_index = 0;
            }

            base.bg(self.main_panel_surface(cx.theme().background))
                .on_action(cx.listener(Self::on_select_prev))
                .on_action(cx.listener(Self::on_select_next))
                .on_action(cx.listener(Self::on_confirm_selection))
                .on_action(cx.listener(Self::on_delete_record))
                .on_key_down(cx.listener(Self::on_key_down))
                .child(render_header(self, cx))
                .child(render_search_input(self, cx))
                .child(self.render_records_list(cx))
                .into_any_element()
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

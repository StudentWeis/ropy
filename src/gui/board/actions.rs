use gpui::{Context, Focusable, Window};

use crate::gui::{active_window, board::RopyBoard, hide_window, panel::settings};

gpui::actions!(
    board,
    [
        Hide,
        Quit,
        Active,
        SelectPrev,
        SelectNext,
        ConfirmSelection,
        DeleteRecord
    ]
);

impl RopyBoard {
    pub fn on_select_prev(&mut self, _: &SelectPrev, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.list_state.scroll_to_reveal_item(self.selected_index);
            cx.notify();
        }
    }

    pub fn on_select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        let count = self.filtered_records.len();
        if count > 0 && self.selected_index < count - 1 {
            self.selected_index += 1;
            self.list_state.scroll_to_reveal_item(self.selected_index);
            cx.notify();
        }
    }

    pub fn on_confirm_selection(
        &mut self,
        _: &ConfirmSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm_record(window, cx, self.selected_index);
    }

    pub fn on_delete_record(
        &mut self,
        _: &DeleteRecord,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(record) = self.filtered_records.get(self.selected_index) {
            let id = record.id;
            self.delete_record(id);
            // Clamp selected_index after deletion
            if self.selected_index > 0
                && self.selected_index >= self.filtered_records.len().saturating_sub(1)
            {
                self.selected_index -= 1;
            }
            cx.notify();
        }
    }

    pub fn on_active_action(&mut self, _: &Active, window: &mut Window, cx: &mut Context<Self>) {
        if self.show_settings && self.hotkey_recording {
            return;
        }

        self.selected_index = 0;
        if self.show_settings {
            settings::reset_settings_dialog(self, window, cx);
        }
        self.list_state.scroll_to_reveal_item(self.selected_index);
        self.show_settings = false;
        self.show_about = false;
        self.show_help = false;
        window.resize(gpui::size(gpui::px(400.), gpui::px(600.)));
        active_window(window, cx);
    }

    pub fn on_hide_action(&mut self, _: &Hide, window: &mut Window, cx: &mut Context<Self>) {
        if self.show_settings && self.hotkey_recording {
            self.cancel_hotkey_recording(window, cx);
            return;
        }

        if self.show_settings {
            settings::reset_settings_dialog(self, window, cx);
            return;
        }
        if self.show_about {
            self.show_about = false;
            cx.notify();
            return;
        }
        if self.show_help {
            self.show_help = false;
            cx.notify();
            return;
        }
        // If the search input is focused, return focus to the main component before hiding
        if let Some(focused_handle) = window.focused(cx)
            && focused_handle == self.search_input.focus_handle(cx)
        {
            window.focus(&self.focus_handle);
            return;
        }
        hide_window(window, cx, self.pinned);
        self.pinned = false;
    }

    #[allow(clippy::unused_self)]
    pub fn on_quit_action(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    pub fn on_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // If the search input is focused, ignore key presses
        if let Some(focused_handle) = window.focused(cx)
            && focused_handle == self.search_input.focus_handle(cx)
        {
            return;
        }

        match event.keystroke.key.as_str() {
            "/" => {
                window.focus(&self.search_input.focus_handle(cx));
            }
            "space" => {
                self.show_preview = !self.show_preview;
                cx.notify();
            }
            "p" => {
                if self.can_toggle_window_pin() {
                    self.toggle_window_pin(window);
                    cx.notify();
                }
            }
            "j" => {
                self.on_select_next(&SelectNext, window, cx);
            }
            "k" => {
                self.on_select_prev(&SelectPrev, window, cx);
            }
            "d" => {
                self.on_delete_record(&DeleteRecord, window, cx);
            }
            "1" => {
                self.confirm_record(window, cx, 0);
            }
            "2" => {
                self.confirm_record(window, cx, 1);
            }
            "3" => {
                self.confirm_record(window, cx, 2);
            }
            "4" => {
                self.confirm_record(window, cx, 3);
            }
            "5" => {
                self.confirm_record(window, cx, 4);
            }
            _ => {}
        }
    }
}

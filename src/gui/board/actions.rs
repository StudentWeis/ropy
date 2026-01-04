use crate::gui::board::RopyBoard;
use crate::gui::{active_window, hide_window};
use gpui::{Context, Focusable, Window};

gpui::actions!(
    board,
    [Hide, Quit, Active, SelectPrev, SelectNext, ConfirmSelection]
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

    pub fn on_active_action(&mut self, _: &Active, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_index = 0;
        self.show_preview = false;
        self.list_state.scroll_to_reveal_item(self.selected_index);
        self.show_settings = false;
        window.resize(gpui::size(gpui::px(400.), gpui::px(600.)));
        active_window(window, cx);
    }

    pub fn on_hide_action(&mut self, _: &Hide, window: &mut Window, cx: &mut Context<Self>) {
        // If still in settings, exit settings view and refocus main board instead of hiding
        if self.show_settings {
            self.show_settings = false;
            self.settings_max_history_input.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
            self.settings_activation_key_input.update(cx, |input, cx| {
                input.set_value("", window, cx);
            });
            window.focus(&self.focus_handle);
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
        hide_window(window, cx);
        self.pinned = false;
    }

    pub fn on_quit_action(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    pub fn on_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // If the "/" key is pressed, focus the search input
        if event.keystroke.key.as_str() == "/" {
            window.focus(&self.search_input.focus_handle(cx));
            return;
        }

        // If the search input is focused, ignore key presses
        if let Some(focused_handle) = window.focused(cx)
            && focused_handle == self.search_input.focus_handle(cx)
        {
            return;
        }

        // If the space key is pressed, toggle preview
        if event.keystroke.key.as_str() == "space" {
            self.show_preview = !self.show_preview;
            cx.notify();
            return;
        }

        // Map number keys to record selection
        let key = &event.keystroke.key;
        let index = match key.as_str() {
            "1" => 0,
            "2" => 1,
            "3" => 2,
            "4" => 3,
            "5" => 4,
            _ => return,
        };
        self.confirm_record(window, cx, index);
    }
}

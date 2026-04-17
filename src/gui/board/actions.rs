use gpui::{Context, Focusable, Window};

use crate::{
    config::LayoutMode,
    gui::{
        active_window,
        board::{ActivePanel, RopyBoard},
        constants::default_window_size,
        hide_window,
        panel::settings,
    },
};

impl RopyBoard {
    /// Clear the search input content and blur it
    pub(crate) fn clear_search(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.search_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        // Blur the search input to remove focus
        window.focus(&self.focus_handle);
    }
}

gpui::actions!(
    board,
    [
        Hide,
        Quit,
        Active,
        SelectLeft,
        SelectRight,
        SelectPrev,
        SelectNext,
        ConfirmSelection,
        ConfirmSelectionPlainText,
        DeleteRecord
    ]
);

pub(super) fn horizontal_grid_target_index(
    selected_index: usize,
    record_count: usize,
    move_right: bool,
    layout_mode: LayoutMode,
) -> Option<usize> {
    if layout_mode != LayoutMode::Grid || record_count == 0 {
        return None;
    }

    if move_right {
        let next_index = selected_index + 1;
        if selected_index.is_multiple_of(2) && next_index < record_count {
            Some(next_index)
        } else {
            None
        }
    } else if !selected_index.is_multiple_of(2) {
        Some(selected_index - 1)
    } else {
        None
    }
}

impl RopyBoard {
    fn move_grid_horizontal(&mut self, move_right: bool, cx: &mut Context<Self>) {
        let count = self.filtered_record_len();
        let Some(next_index) =
            horizontal_grid_target_index(self.selected_index, count, move_right, self.layout_mode)
        else {
            return;
        };

        self.selected_index = next_index;
        self.list_state
            .scroll_to_reveal_item(self.selected_list_index());
        cx.notify();
    }

    pub fn on_select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.move_grid_horizontal(false, cx);
    }

    pub fn on_select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.move_grid_horizontal(true, cx);
    }

    pub fn on_select_prev(&mut self, _: &SelectPrev, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.list_state
                .scroll_to_reveal_item(self.selected_list_index());
            cx.notify();
        }
    }

    pub fn on_select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        let count = self.filtered_record_len();
        if count > 0 && self.selected_index < count - 1 {
            self.selected_index += 1;
            self.list_state
                .scroll_to_reveal_item(self.selected_list_index());
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

    pub fn on_confirm_selection_plain_text(
        &mut self,
        _: &ConfirmSelectionPlainText,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm_record_as_plain_text(window, cx, self.selected_index);
    }

    pub fn on_delete_record(
        &mut self,
        _: &DeleteRecord,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(id) = self.filtered_record_id_at(self.selected_index) {
            self.delete_record(id, cx);
            // Clamp selected_index after deletion
            if self.selected_index > 0
                && self.selected_index >= self.filtered_record_len().saturating_sub(1)
            {
                self.selected_index -= 1;
            }
            cx.notify();
        }
    }

    pub fn on_active_action(&mut self, _: &Active, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_panel == ActivePanel::Settings && self.settings_editor.hotkey_recording {
            return;
        }

        self.selected_index = 0;
        if self.active_panel == ActivePanel::Settings {
            settings::reset_settings_dialog(self, window, cx);
        }
        self.list_state
            .scroll_to_reveal_item(self.selected_list_index());
        self.active_panel = ActivePanel::ClipboardList;
        self.show_clear_confirm = false;
        window.resize(default_window_size());
        active_window(window, cx);
    }

    pub fn on_hide_action(&mut self, _: &Hide, window: &mut Window, cx: &mut Context<Self>) {
        if self.active_panel == ActivePanel::Settings && self.settings_editor.hotkey_recording {
            self.cancel_hotkey_recording(window, cx);
            return;
        }

        if self.show_clear_confirm {
            self.show_clear_confirm = false;
            cx.notify();
            return;
        }

        match self.active_panel {
            ActivePanel::Settings => {
                settings::reset_settings_dialog(self, window, cx);
                return;
            }
            ActivePanel::About | ActivePanel::Help => {
                self.active_panel = ActivePanel::ClipboardList;
                cx.notify();
                return;
            }
            ActivePanel::ClipboardList => {}
        }
        // If the search input is focused, return focus to the main component before hiding
        if let Some(focused_handle) = window.focused(cx)
            && focused_handle == self.search_input.focus_handle(cx)
        {
            window.focus(&self.focus_handle);
            return;
        }
        // Clear search input when hiding the window
        self.clear_search(window, cx);
        hide_window(window, cx, self.pinned);
        if self.pinned {
            self.pinned = false;
            cx.notify();
        }
    }

    #[allow(clippy::unused_self)]
    pub fn on_quit_action(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }

    /// Handle single-character keyboard shortcuts for the clipboard list view.
    ///
    /// These shortcuts are intentionally handled via `on_key_down` rather than
    /// GPUI's `actions!` / `bind_keys` / `on_action` system because they use
    /// single-character keys (`/`, `j`, `k`, `d`, `p`, `1`–`5`, etc.) that
    /// would conflict with normal text input when the search `InputState` is
    /// focused. The focus-guard below ensures these shortcuts are only active
    /// when the main board — not the search input — has focus.
    pub fn on_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Block keyboard shortcuts while the clear-confirm dialog is open
        if self.show_clear_confirm {
            return;
        }

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
            "p" if self.can_toggle_window_pin() => {
                self.toggle_window_pin(window);
                cx.notify();
            }
            "j" => {
                self.on_select_next(&SelectNext, window, cx);
            }
            "k" => {
                self.on_select_prev(&SelectPrev, window, cx);
            }
            "h" => {
                self.on_select_left(&SelectLeft, window, cx);
            }
            "l" => {
                self.on_select_right(&SelectRight, window, cx);
            }
            "d" => {
                self.on_delete_record(&DeleteRecord, window, cx);
            }
            "q" => {
                self.on_hide_action(&Hide, window, cx);
            }
            "f" if let Some(id) = self.filtered_record_id_at(self.selected_index) => {
                self.toggle_record_favorite(id, cx);
                cx.notify();
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

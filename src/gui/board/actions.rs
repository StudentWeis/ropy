use gpui::{Bounds, Context, Focusable, Pixels, Window};

use crate::{
    config::LayoutMode,
    gui::{
        active_window,
        board::{ActivePanel, RopyBoard},
        constants::default_window_size,
        hide_window,
        panel::settings,
        reset_window_geometry_for_activation,
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

#[expect(
    clippy::derive_partial_eq_without_eq,
    reason = "gpui::actions! macro generates PartialEq; we cannot inject Eq from outside"
)]
mod generated_actions {
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
}

pub use generated_actions::*;

pub(super) fn horizontal_grid_target_index(
    selected_index: usize,
    item_bounds: &[Bounds<Pixels>],
    move_right: bool,
    layout_mode: LayoutMode,
) -> Option<usize> {
    if layout_mode != LayoutMode::Grid {
        return None;
    }

    let selected_bounds = item_bounds.get(selected_index)?;
    let selected_center_x = bounds_center_x(selected_bounds);
    let selected_center_y = bounds_center_y(selected_bounds);

    item_bounds
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != selected_index)
        .filter(|(_, bounds)| {
            let center_x = bounds_center_x(bounds);
            if move_right {
                center_x > selected_center_x
            } else {
                center_x < selected_center_x
            }
        })
        .min_by(|(_, left_bounds), (_, right_bounds)| {
            let left_dx = (bounds_center_x(left_bounds) - selected_center_x).abs();
            let right_dx = (bounds_center_x(right_bounds) - selected_center_x).abs();

            left_dx.total_cmp(&right_dx).then_with(|| {
                let left_vertical_distance =
                    (bounds_center_y(left_bounds) - selected_center_y).abs();
                let right_vertical_distance =
                    (bounds_center_y(right_bounds) - selected_center_y).abs();
                left_vertical_distance.total_cmp(&right_vertical_distance)
            })
        })
        .map(|(index, _)| index)
}

fn bounds_center_x(bounds: &Bounds<Pixels>) -> f32 {
    let left: f32 = bounds.left().into();
    let width: f32 = bounds.size.width.into();
    left + width / 2.0
}

fn bounds_center_y(bounds: &Bounds<Pixels>) -> f32 {
    let top: f32 = bounds.top().into();
    let height: f32 = bounds.size.height.into();
    top + height / 2.0
}

fn fallback_horizontal_grid_target_index(
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
        let item_bounds = (0..count)
            .map(|index| self.grid_scroll_handle.bounds_for_item(index))
            .collect::<Option<Vec<_>>>();

        let next_index = item_bounds
            .as_deref()
            .and_then(|bounds| {
                horizontal_grid_target_index(
                    self.selected_index,
                    bounds,
                    move_right,
                    self.layout_mode,
                )
            })
            .or_else(|| {
                fallback_horizontal_grid_target_index(
                    self.selected_index,
                    count,
                    move_right,
                    self.layout_mode,
                )
            });
        let Some(next_index) = next_index else {
            return;
        };

        self.selected_index = next_index;
        self.force_reveal_selected_record();
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
            self.force_reveal_selected_record();
            cx.notify();
        }
    }

    pub fn on_select_next(&mut self, _: &SelectNext, _: &mut Window, cx: &mut Context<Self>) {
        let count = self.filtered_record_len();
        if count > 0 && self.selected_index < count - 1 {
            self.selected_index += 1;
            self.force_reveal_selected_record();
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
            self.force_reveal_selected_record();
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
        self.force_reveal_selected_record();
        self.active_panel = ActivePanel::ClipboardList;
        self.show_clear_confirm = false;
        reset_window_geometry_for_activation(window, default_window_size());
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

    #[expect(clippy::unused_self)]
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

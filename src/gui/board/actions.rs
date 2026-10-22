use gpui_kit::{
    AsKeystroke, Bounds, Context, Focusable, KeyUpEvent, Pixels, Window, component::WindowExt,
};

/// History shortcut catalog, validation and registration.
pub mod shortcuts;
pub(crate) use shortcuts::{
    bind_application_keys, binding_applies_to_history, keybinding_error_message,
};

pub(crate) const QUICK_SELECT_COUNT: usize = 5;

use crate::{
    config::LayoutMode,
    gui::{
        active_window,
        board::{ActivePanel, RopyBoard, search::next_content_filter},
        hide_window,
        panel::settings,
    },
};

impl RopyBoard {
    /// Clear the search input without taking focus from an active dialog.
    pub(crate) fn clear_search(&self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.search_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        if !window.has_active_dialog(cx) {
            window.focus(&self.focus_handle, cx);
        }
    }
}

#[expect(
    clippy::derive_partial_eq_without_eq,
    reason = "gpui_kit::actions! macro generates PartialEq; we cannot inject Eq from outside"
)]
mod generated_actions {
    use gpui_kit as gpui;

    gpui_kit::actions!(
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
            DeleteRecord,
            CycleFilterNext,
            CycleFilterPrev,
            ToggleFavoritesFilter,
            FocusSearch,
            PreviewSelection,
            ToggleWindowPin,
            ToggleFavorite
        ]
    );

    /// Confirm a record by its position in the filtered list.
    #[derive(Clone, Debug, PartialEq, Eq, gpui::Action)]
    #[action(namespace = board, no_json)]
    #[expect(
        clippy::redundant_pub_crate,
        reason = "re-exported at crate visibility with the unit actions"
    )]
    pub(crate) struct ConfirmRecord {
        pub(crate) index: usize,
    }
}

pub(crate) use generated_actions::*;

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
    fn move_grid_horizontal(&mut self, move_right: bool, cx: &mut Context<'_, Self>) {
        let count = self.filtered_record_len();
        let layout = self
            .grid_layout
            .layout(&self.filtered_record_indices, &self.records);
        let item_bounds = (layout.bounds.len() == count).then_some(layout.bounds.as_slice());

        let next_index = item_bounds.map_or_else(
            || {
                fallback_horizontal_grid_target_index(
                    self.selected_index,
                    count,
                    move_right,
                    self.layout_mode,
                )
            },
            |bounds| {
                horizontal_grid_target_index(
                    self.selected_index,
                    bounds,
                    move_right,
                    self.layout_mode,
                )
            },
        );
        let Some(next_index) = next_index else {
            return;
        };

        self.selected_index = next_index;
        self.reveal_selected_record();
        cx.notify();
    }

    pub(crate) fn on_select_left(
        &mut self,
        _: &SelectLeft,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.move_grid_horizontal(false, cx);
    }

    pub(crate) fn on_select_right(
        &mut self,
        _: &SelectRight,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.move_grid_horizontal(true, cx);
    }

    pub(crate) fn on_select_prev(
        &mut self,
        _: &SelectPrev,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.reveal_selected_record();
            cx.notify();
        }
    }

    pub(crate) fn on_select_next(
        &mut self,
        _: &SelectNext,
        _: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let count = self.filtered_record_len();
        if count > 0 && self.selected_index < count - 1 {
            self.selected_index += 1;
            self.reveal_selected_record();
            cx.notify();
        }
    }

    pub(crate) fn on_confirm_selection(
        &mut self,
        _: &ConfirmSelection,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.confirm_record(window, cx, self.selected_index);
    }

    pub(crate) fn on_confirm_selection_plain_text(
        &mut self,
        _: &ConfirmSelectionPlainText,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.confirm_record_as_plain_text(window, cx, self.selected_index);
    }

    pub(crate) fn on_cycle_filter_next(
        &mut self,
        _: &CycleFilterNext,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let next = next_content_filter(self.filter_state.content_filter, true);
        self.filter_state.content_filter = next;
        self.sync_filtered_records_and_reveal(cx);
        cx.notify();
    }

    pub(crate) fn on_cycle_filter_prev(
        &mut self,
        _: &CycleFilterPrev,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        let next = next_content_filter(self.filter_state.content_filter, false);
        self.filter_state.content_filter = next;
        self.sync_filtered_records_and_reveal(cx);
        cx.notify();
    }

    pub(crate) fn on_toggle_favorites_filter(
        &mut self,
        _: &ToggleFavoritesFilter,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.toggle_favorites_only();
        self.sync_filtered_records_and_reveal(cx);
        cx.notify();
    }

    pub(crate) fn on_delete_record(
        &mut self,
        _: &DeleteRecord,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(id) = self.filtered_record_id_at(self.selected_index) {
            let showed_confirm = self.request_delete_record(id, window, cx);
            if !showed_confirm {
                self.reveal_selected_record();
                cx.notify();
            }
        }
    }

    pub(crate) fn on_active_action(
        &mut self,
        _: &Active,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.active_panel == ActivePanel::Settings && self.settings_editor.hotkey.recording {
            return;
        }

        // A resident popup can be hidden and reopened with its dialog still alive.
        if window.has_active_dialog(cx) {
            self.activate_popup(window, cx);
            return;
        }

        self.selected_index = 0;
        self.ui_state.hide_preview();
        if self.active_panel == ActivePanel::Settings {
            settings::reset_settings_dialog(self, window, cx);
        }
        self.reveal_selected_record();
        if !matches!(
            self.active_panel,
            ActivePanel::Shortcuts | ActivePanel::Themes
        ) {
            self.active_panel = ActivePanel::ClipboardList;
        }
        self.cancel_shortcut_recording(cx);
        self.ui_state.clear_confirm = crate::gui::board::ClearConfirmState::Hidden;
        self.activate_popup(window, cx);
    }

    /// Finish activation after the caller has chosen its panel and reset policy.
    pub(crate) fn activate_popup(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.invalidate_copy_confirmation();
        self.activated = true;
        if !window.has_active_dialog(cx) {
            window.focus(&self.focus_handle, cx);
        }
        cx.notify();
        active_window(window, cx);
    }

    pub(crate) fn dismiss_from_hotkey(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.active_panel == ActivePanel::Settings && self.settings_editor.hotkey.recording {
            return;
        }
        self.cancel_shortcut_recording(cx);
        self.ui_state.hide_preview();
        self.invalidate_copy_confirmation();
        self.set_window_pinned(window, false);
        cx.notify();
        self.dismiss_popup(window, cx);
    }

    pub(crate) fn on_hide_action(
        &mut self,
        _: &Hide,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.ui_state.hide_preview() {
            cx.notify();
        }
        if self.active_panel == ActivePanel::Settings && self.settings_editor.hotkey.recording {
            self.cancel_hotkey_recording(window, cx);
            return;
        }

        if self.ui_state.delete_confirm_visible() {
            self.cancel_pending_delete(cx);
            return;
        }

        if self.ui_state.clear_confirm_visible() {
            self.ui_state.clear_confirm = crate::gui::board::ClearConfirmState::Hidden;
            cx.notify();
            return;
        }

        self.invalidate_copy_confirmation();
        match self.active_panel {
            ActivePanel::Themes => {
                self.leave_themes(window, cx);
                return;
            }
            ActivePanel::Shortcuts => {
                self.leave_shortcuts(window, cx);
                return;
            }
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
            window.focus(&self.focus_handle, cx);
            return;
        }
        self.dismiss_popup(window, cx);
    }

    fn dismiss_popup(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.clear_search(window, cx);
        hide_window(window, cx, self.pinned);
        if self.pinned {
            self.set_window_pinned(window, false);
            cx.notify();
        }
    }

    #[expect(clippy::unused_self)]
    pub(crate) fn on_quit_action(
        &mut self,
        _: &Quit,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        cx.quit();
    }

    pub(crate) fn on_focus_search(
        &mut self,
        _: &FocusSearch,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        window.focus(&self.search_input.focus_handle(cx), cx);
    }

    pub(crate) fn on_preview_selection(
        &mut self,
        _: &PreviewSelection,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self
            .ui_state
            .show_space_preview(self.settings_editor.panel_state.space_preview_enabled)
        {
            self.reveal_selected_record();
            cx.notify();
        }
    }

    pub(crate) fn on_toggle_window_pin(
        &mut self,
        _: &ToggleWindowPin,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.toggle_window_pin(window);
        cx.notify();
    }

    pub(crate) fn on_toggle_favorite(
        &mut self,
        _: &ToggleFavorite,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(id) = self.filtered_record_id_at(self.selected_index) {
            self.toggle_record_favorite(id, cx);
            cx.notify();
        }
    }

    pub(crate) fn on_confirm_record(
        &mut self,
        action: &ConfirmRecord,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.confirm_record(window, cx, action.index);
    }

    pub(crate) fn on_key_up(
        &mut self,
        event: &KeyUpEvent,
        _window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        // Focus may have moved into search or another panel since the preview
        // key was pressed. Ignore modifiers on release, which can change while held.
        let released_preview_key = cx
            .key_bindings()
            .borrow()
            .bindings_for_action(&PreviewSelection)
            .any(|binding| {
                binding
                    .keystrokes()
                    .last()
                    .is_some_and(|key| key.as_keystroke().key == event.keystroke.key)
            });
        if released_preview_key && self.ui_state.hide_preview() {
            cx.notify();
        }
    }
}

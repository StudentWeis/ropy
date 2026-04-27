//! Layout-mode selection handler.

use gpui::{Context, Window};

use super::RopyBoard;
use crate::{
    config::{LayoutMode, Settings},
    i18n::I18n,
};

impl RopyBoard {
    pub(crate) fn save_selected_layout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let previous_layout = Settings::read(cx, |s| s.layout.mode);
        let next_layout = LayoutMode::all()
            .get(self.settings_editor.selected_layout)
            .copied()
            .unwrap_or_default();

        if next_layout == previous_layout {
            return;
        }

        if let Err(error_message) = Self::persist_settings_update(cx, |settings| {
            settings.layout.mode = next_layout;
        }) {
            let previous_layout_idx = LayoutMode::all()
                .iter()
                .position(|mode| mode == &previous_layout)
                .unwrap_or_default();
            self.set_layout_selection(previous_layout_idx, window, cx);
            Self::notify_settings_save_failed(window, cx, &error_message);
            cx.notify();
            return;
        }

        self.layout_mode = next_layout;
        self.list_state
            .reset(self.visible_list_len(self.filtered_record_indices.len()));
        self.force_reveal_selected_record();
        Self::notify_settings_success(window, cx, I18n::translate(cx, "settings_save_success"));
        cx.notify();
    }
}

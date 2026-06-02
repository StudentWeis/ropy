/// Action definitions and keyboard handlers for the board.
pub mod actions;
mod clear_confirm;
mod clipboard_ops;
mod color;
mod delete_confirm;
pub(super) mod filtering;
mod header;
mod hotkey_record_handler;
mod preview;
mod record_ops;
mod records_list;
mod render;
/// Search filters and query options for the board.
pub mod search;
mod settings_editor;
mod settings_handler;
mod updater_ui;

#[cfg(test)]
mod tests;

use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

// Re-export utilities for external use
pub(crate) use actions::{
    Active, ConfirmSelection, ConfirmSelectionPlainText, CycleFilterNext, CycleFilterPrev, Hide,
    Quit, SelectLeft, SelectNext, SelectPrev, SelectRight, ToggleFavoritesFilter,
};
use filtering::{ClearConfirmAction, filter_and_sort_record_indices};
use gpui::{
    AppContext, Bounds, Context, Entity, FocusHandle, ListAlignment, ListState, Pixels, Point,
    ScrollHandle, Subscription, Window,
};
use gpui_component::input::InputState;
pub(crate) use search::{ContentFilter, SearchOptions};
use settings_editor::{
    SettingsEditor, UpdateManager, build_language_select, build_layout_select, build_theme_select,
    build_window_opacity_slider,
};

use crate::{
    clipboard::LastCopyState,
    config::{ConfirmMode, LayoutMode, Settings},
    gui::{hide_window, surface_with_opacity, theme::ThemeId},
    i18n::Language,
    repository::SharedRecords,
    utils::read_or_recover,
};

/// Which panel is currently visible in the main window.
///
/// Using an enum instead of multiple boolean flags makes illegal states
/// (e.g. Settings and About both visible) unrepresentable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ActivePanel {
    #[default]
    ClipboardList,
    Settings,
    About,
    Help,
}

pub(super) fn search_query_changed(previous_query: &str, next_query: &str) -> bool {
    previous_query != next_query
}

pub(super) const fn search_query_should_reveal_selection(
    previous_query: &str,
    next_query: &str,
) -> bool {
    previous_query.is_empty() && !next_query.is_empty()
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct FilterState {
    /// Active content type filter
    pub(crate) content_filter: ContentFilter,
    /// Whether to show only favorited records (independent of content type filter)
    pub(crate) favorites_only: bool,
    /// Active text search options
    pub(crate) search_options: SearchOptions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PreviewState {
    #[default]
    Hidden,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DeletionState {
    #[default]
    Idle,
    Deleting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ClearConfirmState {
    #[default]
    Hidden,
    Visible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum DeleteConfirmState {
    #[default]
    Hidden,
    Visible,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct UiState {
    pub(crate) preview: PreviewState,
    pub(crate) deletion: DeletionState,
    pub(crate) clear_confirm: ClearConfirmState,
    pub(crate) delete_confirm: DeleteConfirmState,
}

impl UiState {
    pub(crate) const fn is_deleting_record(self) -> bool {
        matches!(self.deletion, DeletionState::Deleting)
    }

    pub(crate) const fn clear_confirm_visible(self) -> bool {
        matches!(self.clear_confirm, ClearConfirmState::Visible)
    }

    pub(crate) const fn delete_confirm_visible(self) -> bool {
        matches!(self.delete_confirm, DeleteConfirmState::Visible)
    }

    pub(crate) const fn any_overlay_visible(self) -> bool {
        self.clear_confirm_visible() || self.delete_confirm_visible()
    }

    pub(crate) const fn preview_visible(self) -> bool {
        matches!(self.preview, PreviewState::Visible)
    }

    pub(crate) const fn show_preview(&mut self) -> bool {
        if self.preview_visible() {
            return false;
        }

        self.preview = PreviewState::Visible;
        true
    }

    pub(crate) const fn show_space_preview(&mut self, enabled: bool) -> bool {
        if !enabled {
            return false;
        }

        self.show_preview()
    }

    pub(crate) const fn hide_preview(&mut self) -> bool {
        if !self.preview_visible() {
            return false;
        }

        self.preview = PreviewState::Hidden;
        true
    }
}

/// `RopyBoard` Main Window Component
pub(crate) struct RopyBoard {
    pub(crate) records: SharedRecords,
    pub(crate) filtered_record_indices: Arc<Vec<usize>>, // The final shown record indices
    pub(crate) favorite_ids: Arc<HashSet<u64>>,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) _focus_out_subscription: Subscription,
    pub(crate) _search_input_subscription: Subscription,
    pub(crate) search_input: Entity<InputState>,
    pub(crate) last_search_query: String,
    pub(crate) list_state: ListState,
    pub(crate) grid_scroll_handle: ScrollHandle,
    pub(crate) ui_state: UiState,
    pub(crate) selected_index: usize,
    pub(crate) copy_tx: async_channel::Sender<crate::clipboard::CopyRequest>,
    pub(crate) last_copy: Arc<Mutex<LastCopyState>>,
    pub(crate) active_panel: ActivePanel,
    pub(crate) settings_editor: SettingsEditor,
    pub(crate) confirm_mode: ConfirmMode,
    pub(crate) layout_mode: LayoutMode,
    pub(crate) pinned: bool,
    pub(crate) activated: bool,
    pub(crate) hotkey_tx: Option<async_channel::Sender<String>>,
    pub(crate) update_manager: UpdateManager,
    /// Which clear action is currently awaiting confirmation
    clear_confirm_action: ClearConfirmAction,
    /// Record ID awaiting single-record delete confirmation
    pending_delete_id: Option<u64>,
    pub(crate) filter_state: FilterState,
}

impl RopyBoard {
    pub(crate) const fn should_auto_hide_on_focus_out(pinned: bool) -> bool {
        !pinned
    }

    pub(crate) const fn window_pin_available(confirm_mode: ConfirmMode) -> bool {
        matches!(confirm_mode, ConfirmMode::CopyToClipboard)
    }

    const fn resolve_window_pin_state(confirm_mode: ConfirmMode, pinned: bool) -> bool {
        pinned && Self::window_pin_available(confirm_mode)
    }

    pub(crate) const fn can_toggle_window_pin(&self) -> bool {
        Self::window_pin_available(self.confirm_mode)
    }

    pub(crate) fn main_panel_surface(&self, color: gpui::Hsla) -> gpui::Hsla {
        surface_with_opacity(color, self.settings_editor.window_opacity_percent)
    }

    pub(crate) const fn visible_list_len(&self, record_count: usize) -> usize {
        records_list::visible_list_len(record_count, self.layout_mode)
    }

    pub(crate) const fn selected_list_index(&self) -> usize {
        records_list::list_row_for_selected_index(self.selected_index, self.layout_mode)
    }

    fn reveal_selected_grid_item(&self) {
        let viewport_bounds = self.grid_scroll_handle.bounds();
        if viewport_bounds.size.width > gpui::px(0.0)
            && viewport_bounds.size.height > gpui::px(0.0)
            && let Some(item_bounds) = self.grid_scroll_handle.bounds_for_item(self.selected_index)
            && let Some(offset) = grid_reveal_offset(
                viewport_bounds,
                item_bounds,
                self.grid_scroll_handle.offset(),
            )
        {
            self.grid_scroll_handle.set_offset(offset);
            return;
        }

        self.grid_scroll_handle.scroll_to_item(self.selected_index);
    }

    /// Scrolls the active layout's viewport so the currently selected record is
    /// visible. Idempotent when the selection is already on screen: both
    /// `ListState::scroll_to_reveal_item` and `ScrollHandle::scroll_to_item`
    /// (with the default `FirstVisible` strategy) leave the viewport untouched
    /// when the target is already inside it.
    pub(crate) fn reveal_selected_record(&self) {
        if self.filtered_record_indices.is_empty() {
            return;
        }

        match self.layout_mode {
            LayoutMode::List => self
                .list_state
                .scroll_to_reveal_item(self.selected_list_index()),
            LayoutMode::Grid => self.reveal_selected_grid_item(),
        }
    }

    #[expect(clippy::needless_pass_by_ref_mut)]
    pub(crate) fn open_settings_panel(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.active_panel = ActivePanel::Settings;
        self.settings_editor
            .panel_state
            .window_opacity_slider_visible = false;
        window.focus(&self.focus_handle);

        let weak_entity = cx.entity().downgrade();
        window.on_next_frame(move |window, cx| {
            let _ = weak_entity.update(cx, |board, cx| {
                if board.active_panel != ActivePanel::Settings {
                    return;
                }

                board
                    .settings_editor
                    .panel_state
                    .window_opacity_slider_visible = true;
                window.focus(&board.focus_handle);
                cx.notify();
            });
        });
    }

    pub(crate) fn sync_window_opacity_slider(
        &self,
        opacity_percent: u8,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.settings_editor
            .settings_window_opacity_slider
            .update(cx, |slider, cx| {
                slider.set_value(f32::from(opacity_percent), window, cx);
            });
    }

    #[cfg_attr(
        target_os = "macos",
        expect(unused_variables, clippy::missing_const_for_fn)
    )]
    pub(crate) fn set_window_pinned(&mut self, window: &Window, pinned: bool) {
        self.pinned = Self::resolve_window_pin_state(self.confirm_mode, pinned);
        #[cfg(not(target_os = "macos"))]
        crate::gui::utils::set_always_on_top(window, self.pinned);
    }

    pub(crate) fn toggle_window_pin(&mut self, window: &Window) {
        if !self.can_toggle_window_pin() {
            return;
        }

        self.set_window_pinned(window, !self.pinned);
    }

    pub(crate) fn set_confirm_mode(&mut self, confirm_mode: ConfirmMode, window: &Window) {
        self.confirm_mode = confirm_mode;
        if !Self::window_pin_available(confirm_mode) {
            self.set_window_pinned(window, false);
        }
    }

    pub(crate) fn set_hotkey_tx(&mut self, tx: async_channel::Sender<String>) {
        self.hotkey_tx = Some(tx);
    }

    /// Rebuild the tray menu with current i18n translations.
    pub(crate) fn update_tray_menu(cx: &Context<'_, Self>) {
        crate::gui::tray::TrayState::refresh_menu(cx);
    }

    #[expect(clippy::too_many_lines)]
    pub(crate) fn new(
        records: SharedRecords,
        last_copy: Arc<Mutex<LastCopyState>>,
        copy_tx: async_channel::Sender<crate::clipboard::CopyRequest>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);

        // Subscribe to focus out events to hide the window
        let focus_out_subscription =
            cx.on_focus_out(&focus_handle, window, move |this, _event, window, cx| {
                // When the window loses focus, hide the window (unless pinned).
                if Self::should_auto_hide_on_focus_out(this.pinned) {
                    // Clear search input when hiding the window
                    this.clear_search(window, cx);
                    hide_window(window, cx, this.pinned);
                }
            });

        // Read initial values from GPUI Global settings
        let (
            max_history_records,
            max_storage_records,
            activation_key,
            theme,
            language,
            window_opacity_percent,
            layout_mode,
            confirm_mode,
            autostart_enabled,
            auto_check_enabled,
            include_prerelease_enabled,
            hover_preview_enabled,
            space_preview_enabled,
        ) = Settings::read(cx, |s| {
            (
                s.storage.max_history_records,
                s.storage.max_storage_records,
                s.hotkey.activation_key.clone(),
                s.theme.clone(),
                s.language.clone(),
                s.window.opacity_percent,
                s.layout.mode,
                s.confirm.mode,
                s.autostart.enabled,
                s.update.auto_check,
                s.update.include_prerelease,
                s.preview.hover_preview_enabled,
                s.preview.space_preview_enabled,
            )
        });
        let activation_key_placeholder = Self::hotkey_placeholder_text(&activation_key, cx);
        let settings_activation_key_input = cx
            .new(|cx| InputState::new(window, cx).placeholder(activation_key_placeholder.clone()));
        let settings_max_history_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(max_history_records.to_string()));
        let settings_max_storage_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(max_storage_records.to_string()));
        let settings_window_opacity_slider =
            build_window_opacity_slider(window_opacity_percent, window, cx);

        let selected_theme = ThemeId::all()
            .iter()
            .position(|theme_id| theme_id == &theme)
            .unwrap_or_default();
        let theme_select = build_theme_select(selected_theme, window, cx);

        let selected_layout = LayoutMode::all()
            .iter()
            .position(|mode| mode == &layout_mode)
            .unwrap_or_default();
        let layout_select = build_layout_select(selected_layout, window, cx);

        let selected_language = Language::all()
            .iter()
            .position(|lang| lang == &language)
            .unwrap_or(0);
        let language_select = build_language_select(selected_language, window, cx);

        let favorite_ids = Arc::new(Self::load_favorite_ids(cx));
        let search_input = cx.new(|cx| InputState::new(window, cx));
        let initial_filtered_record_indices = {
            let records = read_or_recover(&records);
            filter_and_sort_record_indices(
                &records,
                "",
                ContentFilter::default(),
                SearchOptions::default(),
                favorite_ids.as_ref(),
                false,
            )
        };

        // Render a bit beyond the viewport to reduce scroll-time pop-in while
        // keeping GPUI's lazy list measurement behavior.
        let list_state = ListState::new(
            records_list::visible_list_len(initial_filtered_record_indices.len(), layout_mode),
            ListAlignment::Top,
            gpui::px(160.),
        );
        let search_input_subscription = cx.observe(&search_input, |this, _, cx| {
            let next_query = this.search_input.read(cx).value().to_string();
            if !search_query_changed(&this.last_search_query, &next_query) {
                return;
            }

            let reveal_selection =
                search_query_should_reveal_selection(&this.last_search_query, &next_query);
            this.last_search_query = next_query;

            if reveal_selection {
                this.sync_filtered_records_and_reveal(cx);
            } else {
                this.sync_filtered_records(cx);
            }
            cx.notify();
        });

        let settings_editor = SettingsEditor::new(
            activation_key.clone(),
            activation_key,
            settings_activation_key_input,
            settings_max_history_input,
            settings_max_storage_input,
            settings_window_opacity_slider,
            selected_theme,
            theme_select,
            selected_layout,
            layout_select,
            window_opacity_percent,
            selected_language,
            language_select,
            autostart_enabled,
            auto_check_enabled,
            include_prerelease_enabled,
            hover_preview_enabled,
            space_preview_enabled,
        );

        Self {
            records,
            focus_handle,
            _focus_out_subscription: focus_out_subscription,
            _search_input_subscription: search_input_subscription,
            search_input,
            last_search_query: String::new(),
            grid_scroll_handle: ScrollHandle::new(),
            ui_state: UiState::default(),
            selected_index: 0,
            last_copy,
            list_state,
            filtered_record_indices: Arc::new(initial_filtered_record_indices),
            favorite_ids,
            copy_tx,
            active_panel: ActivePanel::default(),
            settings_editor,
            confirm_mode,
            layout_mode,
            pinned: false,
            activated: false,
            hotkey_tx: None,
            update_manager: UpdateManager::new(),
            clear_confirm_action: ClearConfirmAction::AllHistory,
            pending_delete_id: None,
            filter_state: FilterState::default(),
        }
    }
}

fn grid_reveal_offset(
    viewport_bounds: Bounds<Pixels>,
    item_bounds: Bounds<Pixels>,
    current_offset: Point<Pixels>,
) -> Option<Point<Pixels>> {
    let mut next_offset = current_offset;
    let mut changed = false;

    if item_bounds.top() + next_offset.y < viewport_bounds.top() {
        next_offset.y = viewport_bounds.top() - item_bounds.top();
        changed = true;
    } else if item_bounds.bottom() + next_offset.y > viewport_bounds.bottom() {
        next_offset.y = viewport_bounds.bottom() - item_bounds.bottom();
        changed = true;
    }

    if item_bounds.left() + next_offset.x < viewport_bounds.left() {
        next_offset.x = viewport_bounds.left() - item_bounds.left();
        changed = true;
    } else if item_bounds.right() + next_offset.x > viewport_bounds.right() {
        next_offset.x = viewport_bounds.right() - item_bounds.right();
        changed = true;
    }

    changed.then_some(next_offset)
}

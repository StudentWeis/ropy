mod actions;
mod clear_confirm;
mod header;
mod hotkey_record_handler;
mod preview;
mod records_list;
mod search;
mod settings_handler;
mod updater_ui;

use std::{
    collections::HashSet,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};

// Re-export utilities for external use
pub use actions::{Active, ConfirmSelection, Hide, Quit, SelectNext, SelectPrev};
use gpui::{
    AnyElement, App, AppContext, Context, Entity, FocusHandle, ListAlignment, ListState, Render,
    SharedString, Subscription, Window,
    prelude::{FluentBuilder, InteractiveElement, IntoElement, ParentElement, Styled},
};
use gpui_component::{
    ActiveTheme, IndexPath, WindowExt,
    input::InputState,
    select::{SelectEvent, SelectState},
    v_flex,
};
use header::render_header;
pub use search::{ContentFilter, SearchOptions};
use search::{filter_records_by_query, render_search_input};

use crate::{
    clipboard::LastCopyState,
    config::{ConfirmMode, Settings},
    gui::{
        hide_window,
        panel::{
            about::render_about_content, help::render_help_content,
            settings::render_settings_content,
        },
        theme::ThemeId,
    },
    i18n::Language,
    repository::{
        ClipboardRecord, ClipboardRepository, GlobalRepository, SharedRecords, models::ContentType,
    },
    updater::models::UpdateStatus,
    utils::{deserialize_file_paths, lock_or_recover, read_or_recover, write_or_recover},
};

fn build_copy_request(
    content: &str,
    content_type: &ContentType,
    completion: Option<mpsc::Sender<()>>,
) -> Option<crate::clipboard::CopyRequest> {
    match content_type {
        ContentType::Text => Some(completion.map_or_else(
            || crate::clipboard::CopyRequest::text(content.to_string()),
            |tx| crate::clipboard::CopyRequest::text_with_completion(content.to_string(), tx),
        )),
        ContentType::Image => Some(completion.map_or_else(
            || crate::clipboard::CopyRequest::image(content.to_string()),
            |tx| crate::clipboard::CopyRequest::image_with_completion(content.to_string(), tx),
        )),
        ContentType::FilePath => {
            let paths = deserialize_file_paths(content);
            if paths.is_empty() {
                None
            } else {
                Some(if let Some(tx) = completion {
                    crate::clipboard::CopyRequest::files_with_completion(paths, tx)
                } else {
                    crate::clipboard::CopyRequest::files(paths)
                })
            }
        }
    }
}

fn filter_and_sort_record_indices(
    records: &[ClipboardRecord],
    query: &str,
    content_filter: ContentFilter,
    search_options: SearchOptions,
    favorite_ids: &HashSet<u64>,
    favorites_only: bool,
) -> Vec<usize> {
    let mut filtered_indices = filter_records_by_query(
        records,
        query,
        content_filter,
        search_options,
        favorite_ids,
        favorites_only,
    );

    filtered_indices.sort_unstable_by(|left_index, right_index| {
        let left = records.get(*left_index);
        let right = records.get(*right_index);

        match (left, right) {
            (Some(left), Some(right)) => ClipboardRepository::compare_for_display(left, right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left_index.cmp(right_index),
        }
    });

    filtered_indices
}

/// `RopyBoard` Main Window Component
#[allow(clippy::struct_excessive_bools)]
pub struct RopyBoard {
    pub(crate) records: SharedRecords,
    pub(crate) filtered_record_indices: Arc<Vec<usize>>, // The final shown record indices
    pub(crate) favorite_ids: Arc<HashSet<u64>>,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) _focus_out_subscription: Subscription,
    pub(crate) search_input: Entity<InputState>,
    pub(crate) list_state: ListState,
    pub(crate) selected_index: usize,
    pub(crate) copy_tx: async_channel::Sender<crate::clipboard::CopyRequest>,
    pub(crate) last_copy: Arc<Mutex<LastCopyState>>,
    pub(crate) show_settings: bool,
    pub(crate) show_about: bool,
    pub(crate) show_help: bool,
    pub(crate) show_preview: bool,
    pub(crate) hotkey_recording: bool,
    pub(crate) hotkey_manual_editing: bool,
    pub(crate) pending_hotkey: String,
    pub(crate) hotkey_before_recording: String,
    pub(crate) settings_activation_key_input: Entity<InputState>,
    pub(crate) settings_max_history_input: Entity<InputState>,
    pub(crate) settings_max_storage_input: Entity<InputState>,
    pub(crate) selected_theme: usize, // Index into ThemeId::all()
    pub(crate) theme_select: Entity<SelectState<Vec<SharedString>>>,
    pub(crate) autostart_enabled: bool,
    pub(crate) confirm_mode: ConfirmMode,
    pub(crate) pinned: bool,
    pub(crate) hotkey_tx: Option<async_channel::Sender<String>>,
    pub(crate) selected_language: usize, // Index into Language::all()
    pub(crate) language_select: Entity<SelectState<Vec<SharedString>>>,
    /// Track if we're in a delete operation to preserve scroll position
    pub(crate) deleting_record: bool,
    /// Current auto-update status
    pub(crate) update_status: UpdateStatus,
    /// Whether auto-check for updates is enabled (mirrors settings)
    pub(crate) auto_check_enabled: bool,
    /// Whether hover preview is enabled (mirrors settings)
    pub(crate) hover_preview_enabled: bool,
    /// Whether the clear-all confirmation dialog is visible
    pub(crate) show_clear_confirm: bool,
    /// Active content type filter
    pub(crate) content_filter: ContentFilter,
    /// Whether to show only favorited records (independent of content type filter)
    pub(crate) favorites_only: bool,
    /// Active text search options
    pub(crate) search_options: SearchOptions,
}

impl RopyBoard {
    pub(crate) const fn window_pin_available(confirm_mode: ConfirmMode) -> bool {
        matches!(confirm_mode, ConfirmMode::CopyToClipboard)
    }

    const fn resolve_window_pin_state(confirm_mode: ConfirmMode, pinned: bool) -> bool {
        pinned && Self::window_pin_available(confirm_mode)
    }

    pub(crate) const fn can_toggle_window_pin(&self) -> bool {
        Self::window_pin_available(self.confirm_mode)
    }

    #[allow(clippy::missing_const_for_fn)]
    pub(crate) fn set_window_pinned(&mut self, _window: &Window, pinned: bool) {
        self.pinned = Self::resolve_window_pin_state(self.confirm_mode, pinned);
        #[cfg(not(target_os = "macos"))]
        crate::gui::utils::set_always_on_top(_window, self.pinned);
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

    pub fn set_hotkey_tx(&mut self, tx: async_channel::Sender<String>) {
        self.hotkey_tx = Some(tx);
    }

    fn load_favorite_ids(cx: &App) -> HashSet<u64> {
        GlobalRepository::read(cx, |repo| {
            repo.and_then(|repo| repo.favorite_ids().ok())
                .map(|ids| ids.into_iter().collect())
                .unwrap_or_default()
        })
    }

    fn refresh_records_from_repository(&mut self, cx: &Context<Self>) {
        let max_history_records = Settings::read(cx, |s| s.storage.max_history_records);

        GlobalRepository::read(cx, |repo| {
            let Some(repo) = repo else {
                return;
            };

            match repo.get_display_records(max_history_records) {
                Ok(records) => {
                    let mut guard = write_or_recover(&self.records);
                    *guard = records;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to reload display records");
                }
            }

            match repo.favorite_ids() {
                Ok(ids) => {
                    self.favorite_ids = Arc::new(ids.into_iter().collect());
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to reload favorite ids");
                }
            }
        });
    }

    /// Rebuild the tray menu with current i18n translations.
    pub(crate) fn update_tray_menu(cx: &Context<Self>) {
        crate::gui::tray::TrayState::refresh_menu(cx);
    }

    #[allow(clippy::too_many_lines)]
    pub fn new(
        records: SharedRecords,
        last_copy: Arc<Mutex<LastCopyState>>,
        copy_tx: async_channel::Sender<crate::clipboard::CopyRequest>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);

        // Subscribe to focus out events to hide the window
        let focus_out_subscription =
            cx.on_focus_out(&focus_handle, window, move |this, _event, window, cx| {
                // When the window loses focus, hide the window.
                // Do NOT hide while settings panels are open — their popups
                // (Select dropdowns, overlays) steal focus and would cause the window
                // to disappear and become un-reopenable.
                if !this.pinned && !this.show_settings {
                    // Clear search input when hiding the window
                    this.clear_search(window, cx);
                    hide_window(window, cx, this.pinned);
                }
            });

        // Render a bit beyond the viewport to reduce scroll-time pop-in while
        // keeping GPUI's lazy list measurement behavior.
        let list_state = ListState::new(0, ListAlignment::Top, gpui::px(160.));

        // Read initial values from GPUI Global settings
        let (
            max_history_records,
            max_storage_records,
            activation_key,
            theme,
            language,
            confirm_mode,
            autostart_enabled,
            auto_check_enabled,
            hover_preview_enabled,
        ) = Settings::read(cx, |s| {
            (
                s.storage.max_history_records,
                s.storage.max_storage_records,
                s.hotkey.activation_key.clone(),
                s.theme.clone(),
                s.language.clone(),
                s.confirm.mode,
                s.autostart.enabled,
                s.update.auto_check,
                s.preview.hover_preview_enabled,
            )
        });
        let settings_activation_key_input = cx.new(|cx| InputState::new(window, cx));
        let settings_max_history_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(max_history_records.to_string()));
        let settings_max_storage_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(max_storage_records.to_string()));

        let selected_theme = ThemeId::all()
            .iter()
            .position(|theme_id| theme_id == &theme)
            .unwrap_or_default();

        let theme_items: Vec<SharedString> = ThemeId::all()
            .iter()
            .map(|theme_id| SharedString::from(theme_id.display_name()))
            .collect();
        let theme_select = cx.new(|cx| {
            SelectState::new(
                theme_items,
                Some(IndexPath::default().row(selected_theme)),
                window,
                cx,
            )
        });
        cx.subscribe_in(
            &theme_select,
            window,
            |this, _entity, event: &SelectEvent<Vec<SharedString>>, _window, cx| {
                if let SelectEvent::Confirm(Some(val)) = event {
                    let themes = ThemeId::all();
                    if let Some(idx) = themes
                        .iter()
                        .position(|theme| theme.display_name() == val.as_ref())
                    {
                        this.selected_theme = idx;
                        cx.notify();
                    }
                }
            },
        )
        .detach();

        let selected_language = Language::all()
            .iter()
            .position(|lang| lang == &language)
            .unwrap_or(0);

        // Create language select dropdown
        let language_items: Vec<SharedString> = Language::all()
            .iter()
            .map(|l| SharedString::from(l.display_name()))
            .collect();
        let language_select = cx.new(|cx| {
            SelectState::new(
                language_items,
                Some(IndexPath::default().row(selected_language)),
                window,
                cx,
            )
        });
        cx.subscribe_in(
            &language_select,
            window,
            |this, _entity, event: &SelectEvent<Vec<SharedString>>, _window, cx| {
                if let SelectEvent::Confirm(Some(val)) = event {
                    let langs = Language::all();
                    if let Some(idx) = langs.iter().position(|l| l.display_name() == val.as_ref()) {
                        this.selected_language = idx;
                        cx.notify();
                    }
                }
            },
        )
        .detach();

        let search_input = cx.new(|cx| InputState::new(window, cx));
        let favorite_ids = Arc::new(Self::load_favorite_ids(cx));

        Self {
            records,
            focus_handle,
            _focus_out_subscription: focus_out_subscription,
            search_input,
            selected_index: 0,
            last_copy,
            list_state,
            filtered_record_indices: Arc::new(Vec::new()),
            favorite_ids,
            copy_tx,
            show_settings: false,
            show_about: false,
            show_help: false,
            show_preview: false,
            hotkey_recording: false,
            hotkey_manual_editing: false,
            pending_hotkey: activation_key.clone(),
            hotkey_before_recording: activation_key,
            settings_activation_key_input,
            settings_max_history_input,
            settings_max_storage_input,
            selected_theme,
            theme_select,
            autostart_enabled,
            confirm_mode,
            pinned: false,
            hotkey_tx: None,
            selected_language,
            language_select,
            deleting_record: false,
            update_status: UpdateStatus::Idle,
            auto_check_enabled,
            hover_preview_enabled,
            show_clear_confirm: false,
            content_filter: ContentFilter::default(),
            favorites_only: false,
            search_options: SearchOptions::default(),
        }
    }

    /// Write confirmed content to the clipboard before the confirm action completes.
    fn write_content_to_clipboard(&self, content: &str, content_type: &ContentType) -> bool {
        let completion = self
            .confirm_mode
            .requires_clipboard_completion()
            .then(mpsc::channel);
        let request = build_copy_request(
            content,
            content_type,
            completion.as_ref().map(|(tx, _)| tx.clone()),
        );

        if let Some(req) = request {
            if self.copy_tx.send_blocking(req).is_err() {
                tracing::warn!("failed to send clipboard write request");
                return false;
            }
            if let Some((_, rx)) = completion
                && rx.recv_timeout(Duration::from_millis(500)).is_err()
            {
                tracing::warn!("timed out waiting for clipboard write completion");
                return false;
            }
            return true;
        }

        false
    }

    /// Clear clipboard history
    pub(crate) fn clear_history(&mut self, cx: &Context<Self>) {
        GlobalRepository::read(cx, |repo| {
            if let Some(repo) = repo {
                if let Err(e) = repo.clear() {
                    tracing::warn!(error = %e, "failed to clear clipboard history");
                } else {
                    {
                        let mut guard = write_or_recover(&self.records);
                        guard.clear();
                    }
                    self.favorite_ids = Arc::new(HashSet::new());
                }
            }
        });
    }

    /// Clear last copy state
    pub(crate) fn clear_last_copy_state(&self) {
        let mut guard = lock_or_recover(&self.last_copy);
        *guard = LastCopyState::Text(String::new());
    }

    /// Delete a single record by ID
    pub fn delete_record(&mut self, id: u64, cx: &Context<Self>) {
        GlobalRepository::read(cx, |repo| {
            if let Some(repo) = repo {
                if let Err(e) = repo.delete(id) {
                    tracing::warn!(error = %e, "failed to delete clipboard record");
                } else {
                    // Mark that we're in a delete operation to preserve scroll position
                    self.deleting_record = true;
                    self.refresh_records_from_repository(cx);
                }
            }
        });
    }

    /// Toggle favorite state of a record.
    pub fn toggle_record_favorite(&mut self, id: u64, cx: &Context<Self>) {
        GlobalRepository::read(cx, |repo| {
            let Some(repo) = repo else {
                return;
            };
            match repo.toggle_favorite(id) {
                Ok(_) => {
                    self.refresh_records_from_repository(cx);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to toggle favorite on clipboard record");
                }
            }
        });
    }

    /// Toggle pin state of a record
    pub fn toggle_record_pin(&mut self, id: u64, cx: &Context<Self>) {
        GlobalRepository::read(cx, |repo| {
            let Some(repo) = repo else {
                return;
            };
            if let Err(e) = repo.toggle_pin(id) {
                tracing::warn!(error = %e, "failed to toggle pin on clipboard record");
                return;
            }
            self.refresh_records_from_repository(cx);
        });
    }

    /// Toggle the content type filter. Clicking the same filter again resets to All.
    pub(crate) fn toggle_content_filter(&mut self, target: ContentFilter) {
        if self.content_filter == target {
            self.content_filter = ContentFilter::All;
        } else {
            self.content_filter = target;
        }
    }

    /// Toggle the favorites-only filter (independent of content type filter).
    pub(crate) const fn toggle_favorites_only(&mut self) {
        self.favorites_only = !self.favorites_only;
    }

    pub(crate) const fn toggle_case_sensitive_search(&mut self) {
        self.search_options.case_sensitive = !self.search_options.case_sensitive;
    }

    pub(crate) const fn toggle_whole_word_search(&mut self) {
        self.search_options.whole_word = !self.search_options.whole_word;
    }

    /// Get filtered records based on search query and content type filter
    fn get_filtered_record_indices(&self, query: &str) -> Vec<usize> {
        let records = read_or_recover(&self.records);
        filter_and_sort_record_indices(
            &records,
            query,
            self.content_filter,
            self.search_options,
            &self.favorite_ids,
            self.favorites_only,
        )
    }

    pub(crate) fn filtered_record_len(&self) -> usize {
        self.filtered_record_indices.len()
    }

    pub(crate) fn filtered_record_index_at(&self, index: usize) -> Option<usize> {
        self.filtered_record_indices.get(index).copied()
    }

    pub(crate) fn filtered_record_id_at(&self, index: usize) -> Option<u64> {
        let records = read_or_recover(&self.records);
        let record_index = self.filtered_record_index_at(index)?;
        records.get(record_index).map(|record| record.id)
    }

    /// Confirm selection: copy record to clipboard and hide.
    /// The clipboard listener will re-capture the copy event and the
    /// repository layer handles deduplication via content hash upsert.
    pub(crate) fn confirm_record(&self, window: &mut Window, cx: &Context<Self>, index: usize) {
        let (content, content_type) = {
            let Some(record_index) = self.filtered_record_index_at(index) else {
                return;
            };
            let record = {
                let records = read_or_recover(&self.records);
                records.get(record_index).cloned()
            };
            let Some(record) = record else {
                tracing::warn!(
                    index = record_index,
                    "failed to resolve filtered record from cache"
                );
                return;
            };
            (record.content, record.content_type)
        };

        if !self.write_content_to_clipboard(&content, &content_type) {
            return;
        }

        match self.confirm_mode {
            ConfirmMode::CopyToClipboard => {
                if !self.pinned {
                    hide_window(window, cx, self.pinned);
                }
            }
            ConfirmMode::PasteImmediately => {
                hide_window(window, cx, false);
                if let Err(error) = crate::gui::paste::trigger_paste() {
                    tracing::warn!(error = %error, "failed to trigger immediate paste");
                }
            }
        }
    }
}

impl Render for RopyBoard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let base = v_flex()
            .id("ropy-board")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_hide_action))
            .on_action(cx.listener(Self::on_quit_action))
            .on_action(cx.listener(Self::on_active_action))
            .bg(cx.theme().background)
            .size_full()
            .px_4()
            .pb_4();

        let body: AnyElement = if self.show_settings {
            base.on_key_down(cx.listener(Self::on_settings_key_down))
                .child(render_settings_content(self, cx))
                .into_any_element()
        } else if self.show_about {
            base.child(render_about_content(self, cx))
                .into_any_element()
        } else if self.show_help {
            base.child(render_help_content(self, cx)).into_any_element()
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

            base.on_action(cx.listener(Self::on_select_prev))
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
        // This bypasses NotificationList (which hardcodes top_4/right_4) so we can
        // freely control position, spacing, and opacity.
        let notifs: Vec<_> = window.notifications(cx).iter().cloned().collect();
        let has_notifs = !notifs.is_empty();
        let show_clear_confirm = self.show_clear_confirm;
        gpui::div()
            .relative()
            .size_full()
            .child(body)
            .when(show_clear_confirm, |this| {
                this.child(clear_confirm::render_clear_confirm_overlay(cx))
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

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};

    use super::*;
    use crate::config::ConfirmMode;

    fn test_datetime(hour: u32) -> chrono::DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 3, 31, hour, 0, 0)
            .single()
            .unwrap_or_else(|| panic!("invalid local datetime for test hour {hour}"))
    }

    fn test_record(
        id: u64,
        content: &str,
        content_type: ContentType,
        pinned: bool,
        created_at: chrono::DateTime<Local>,
    ) -> ClipboardRecord {
        ClipboardRecord {
            id,
            content: content.to_string(),
            content_type,
            pinned,
            created_at,
        }
    }

    #[test]
    fn test_window_pin_availability_depends_on_confirm_mode() {
        assert!(RopyBoard::window_pin_available(
            ConfirmMode::CopyToClipboard
        ));
        assert!(!RopyBoard::window_pin_available(
            ConfirmMode::PasteImmediately,
        ));
    }

    #[test]
    fn test_resolve_window_pin_state_disables_pin_for_immediate_paste() {
        assert!(RopyBoard::resolve_window_pin_state(
            ConfirmMode::CopyToClipboard,
            true,
        ));
        assert!(!RopyBoard::resolve_window_pin_state(
            ConfirmMode::PasteImmediately,
            true,
        ));
        assert!(!RopyBoard::resolve_window_pin_state(
            ConfirmMode::PasteImmediately,
            false,
        ));
    }

    #[test]
    fn test_toggle_content_filter() {
        // Toggling the same filter twice returns to All
        let mut filter = ContentFilter::All;

        // Simulate toggle to Text
        filter = if filter == ContentFilter::Text {
            ContentFilter::All
        } else {
            ContentFilter::Text
        };
        assert_eq!(filter, ContentFilter::Text);

        // Simulate toggle Text again -> back to All
        filter = if filter == ContentFilter::Text {
            ContentFilter::All
        } else {
            ContentFilter::Text
        };
        assert_eq!(filter, ContentFilter::All);

        // Simulate toggle to Image
        filter = if filter == ContentFilter::Image {
            ContentFilter::All
        } else {
            ContentFilter::Image
        };
        assert_eq!(filter, ContentFilter::Image);

        // Simulate toggle to Files
        filter = if filter == ContentFilter::Files {
            ContentFilter::All
        } else {
            ContentFilter::Files
        };
        assert_eq!(filter, ContentFilter::Files);

        // Simulate toggle to Text while Image is active -> switches to Text
        filter = if filter == ContentFilter::Text {
            ContentFilter::All
        } else {
            ContentFilter::Text
        };
        assert_eq!(filter, ContentFilter::Text);
    }

    #[test]
    fn test_filter_and_sort_record_indices_display_order_returns_sorted_indices() {
        let records = vec![
            test_record(1, "alpha", ContentType::Text, false, test_datetime(9)),
            test_record(2, "beta", ContentType::Text, true, test_datetime(8)),
            test_record(3, "alphabet", ContentType::Text, true, test_datetime(10)),
            test_record(4, "gamma", ContentType::Image, false, test_datetime(11)),
        ];

        let indices = filter_and_sort_record_indices(
            &records,
            "alp",
            ContentFilter::All,
            SearchOptions::default(),
            &HashSet::new(),
            false,
        );

        assert_eq!(indices, vec![2, 0]);
    }

    #[test]
    fn test_filter_and_sort_record_indices_image_filter_ignores_query() {
        let records = vec![
            test_record(1, "hello", ContentType::Text, false, test_datetime(9)),
            test_record(2, "image-a", ContentType::Image, false, test_datetime(8)),
            test_record(3, "image-b", ContentType::Image, true, test_datetime(10)),
        ];

        let indices = filter_and_sort_record_indices(
            &records,
            "hello",
            ContentFilter::Image,
            SearchOptions::default(),
            &HashSet::new(),
            false,
        );

        assert_eq!(indices, vec![2, 1]);
    }

    #[test]
    fn test_filter_and_sort_record_indices_files_filter_matches_file_records() {
        let records = vec![
            test_record(
                1,
                "[\"/tmp/notes.txt\"]",
                ContentType::FilePath,
                false,
                test_datetime(9),
            ),
            test_record(2, "hello", ContentType::Text, false, test_datetime(8)),
            test_record(
                3,
                "[\"/tmp/archive.zip\"]",
                ContentType::FilePath,
                true,
                test_datetime(10),
            ),
        ];

        let indices = filter_and_sort_record_indices(
            &records,
            "",
            ContentFilter::Files,
            SearchOptions::default(),
            &HashSet::new(),
            false,
        );

        assert_eq!(indices, vec![2, 0]);
    }

    #[test]
    fn test_build_copy_request_when_file_payload_is_json_returns_files_request() {
        let request = build_copy_request(
            "[\"/tmp/alpha.txt\",\"/tmp/beta.txt\"]",
            &ContentType::FilePath,
            None,
        );

        match request {
            Some(crate::clipboard::CopyRequest::Files { paths, completion }) => {
                assert_eq!(paths, vec!["/tmp/alpha.txt", "/tmp/beta.txt"]);
                assert!(completion.is_none());
            }
            _ => panic!("expected files copy request"),
        }
    }

    #[test]
    fn test_build_copy_request_when_file_payload_is_legacy_string_returns_single_file() {
        let request = build_copy_request("/tmp/legacy.txt", &ContentType::FilePath, None);

        match request {
            Some(crate::clipboard::CopyRequest::Files { paths, completion }) => {
                assert_eq!(paths, vec!["/tmp/legacy.txt"]);
                assert!(completion.is_none());
            }
            _ => panic!("expected files copy request"),
        }
    }
}

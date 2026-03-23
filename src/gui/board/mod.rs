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
    sync::{Arc, Mutex, PoisonError, mpsc},
    time::Duration,
};

// Re-export utilities for external use
pub use actions::{Active, ConfirmSelection, Hide, Quit, SelectNext, SelectPrev};
use gpui::{
    AnyElement, AppContext, Context, Entity, FocusHandle, ListAlignment, ListState, Render,
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
    },
    i18n::{I18n, Language},
    repository::{ClipboardRecord, ClipboardRepository, models::ContentType},
    updater::models::UpdateStatus,
};

/// `RopyBoard` Main Window Component
#[allow(clippy::struct_excessive_bools)]
pub struct RopyBoard {
    pub(crate) records: Arc<Mutex<Vec<ClipboardRecord>>>,
    pub(crate) filtered_records: Arc<Vec<ClipboardRecord>>, // The final shown records
    pub(crate) repository: Option<Arc<ClipboardRepository>>,
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
    pub(crate) selected_theme: usize, // 0: Light, 1: Dark, 2: System
    pub(crate) autostart_enabled: bool,
    pub(crate) confirm_mode: ConfirmMode,
    pub(crate) pinned: bool,
    pub(crate) hotkey_tx: Option<async_channel::Sender<String>>,
    // I18n
    pub(crate) i18n: I18n,
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
    /// Active text search options
    pub(crate) search_options: SearchOptions,
    /// System tray icon handle for menu updates on language change
    pub(crate) tray_icon: Option<tray_icon::TrayIcon>,
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

    pub fn set_tray_icon(&mut self, tray_icon: Option<tray_icon::TrayIcon>) {
        self.tray_icon = tray_icon;
    }

    /// Rebuild the tray menu with current i18n translations.
    pub(crate) fn update_tray_menu(&self) {
        if let Some(ref tray) = self.tray_icon {
            match crate::gui::tray::build_tray_menu(&self.i18n) {
                Ok(menu) => tray.set_menu(Some(Box::new(menu))),
                Err(e) => tracing::warn!(error = %e, "failed to rebuild tray menu"),
            }
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn new(
        records: Arc<Mutex<Vec<ClipboardRecord>>>,
        repository: Option<Arc<ClipboardRepository>>,
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

        // Measure all items initially so scrollbar thumb size is stable on first paint.
        let list_state = ListState::new(0, ListAlignment::Top, gpui::px(100.)).measure_all();

        // Read initial values from GPUI Global settings
        let (
            max_history_records,
            max_storage_records,
            activation_key,
            theme_index,
            language,
            confirm_mode,
            autostart_enabled,
            auto_check_enabled,
            hover_preview_enabled,
        ) = Settings::read(cx, |s| {
            let theme_idx = match s.theme {
                crate::config::AppTheme::Light => 0,
                crate::config::AppTheme::Dark => 1,
                crate::config::AppTheme::System => 2,
            };
            (
                s.storage.max_history_records,
                s.storage.max_storage_records,
                s.hotkey.activation_key.clone(),
                theme_idx,
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

        // Initialize I18n with the language from settings
        let i18n = I18n::new(language.clone()).unwrap_or_else(|e| {
            tracing::warn!(error = %e, language = %language.code(), "failed to load i18n for board; falling back to default");
            I18n::default()
        });
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

        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(i18n.t("search_placeholder")));

        Self {
            records,
            repository,
            focus_handle,
            _focus_out_subscription: focus_out_subscription,
            search_input,
            selected_index: 0,
            last_copy,
            list_state,
            filtered_records: Arc::new(Vec::new()),
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
            selected_theme: theme_index,
            autostart_enabled,
            confirm_mode,
            pinned: false,
            hotkey_tx: None,
            i18n,
            selected_language,
            language_select,
            deleting_record: false,
            update_status: UpdateStatus::Idle,
            auto_check_enabled,
            hover_preview_enabled,
            show_clear_confirm: false,
            content_filter: ContentFilter::default(),
            search_options: SearchOptions::default(),
            tray_icon: None,
        }
    }

    /// Write confirmed content to the clipboard before the confirm action completes.
    fn write_content_to_clipboard(&self, content: &str, content_type: &ContentType) -> bool {
        let completion = self
            .confirm_mode
            .requires_clipboard_completion()
            .then(mpsc::channel);
        let request = match content_type {
            ContentType::Text => Some(match completion.as_ref() {
                Some((tx, _)) => crate::clipboard::CopyRequest::text_with_completion(
                    content.to_string(),
                    tx.clone(),
                ),
                None => crate::clipboard::CopyRequest::text(content.to_string()),
            }),
            ContentType::Image => Some(match completion.as_ref() {
                Some((tx, _)) => crate::clipboard::CopyRequest::image_with_completion(
                    content.to_string(),
                    tx.clone(),
                ),
                None => crate::clipboard::CopyRequest::image(content.to_string()),
            }),
            ContentType::FilePath => todo!(),
        };

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
    pub(crate) fn clear_history(&self) {
        if let Some(ref repo) = self.repository {
            if let Err(e) = repo.clear() {
                tracing::warn!(error = %e, "failed to clear clipboard history");
            } else {
                let mut guard = self.records.lock().unwrap_or_else(PoisonError::into_inner);
                guard.clear();
            }
        }
    }

    /// Clear last copy state
    pub(crate) fn clear_last_copy_state(&self) {
        match self.last_copy.lock() {
            Ok(mut guard) => {
                *guard = LastCopyState::Text(String::new());
            }
            Err(poisoned) => {
                *poisoned.into_inner() = LastCopyState::Text(String::new());
            }
        }
    }

    /// Delete a single record by ID
    pub fn delete_record(&mut self, id: u64) {
        if let Some(ref repo) = self.repository {
            if let Err(e) = repo.delete(id) {
                tracing::warn!(error = %e, "failed to delete clipboard record");
            } else {
                self.records
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .retain(|record| record.id != id);
                // Mark that we're in a delete operation to preserve scroll position
                self.deleting_record = true;
            }
        }
    }

    /// Toggle pin state of a record
    pub fn toggle_record_pin(&self, id: u64) {
        let Some(ref repo) = self.repository else {
            return;
        };
        if let Err(e) = repo.toggle_pin(id) {
            tracing::warn!(error = %e, "failed to toggle pin on clipboard record");
            return;
        }
        let mut guard = self.records.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(record) = guard.iter_mut().find(|r| r.id == id) {
            record.pinned = !record.pinned;
        }
    }

    /// Toggle the content type filter. Clicking the same filter again resets to All.
    pub(crate) fn toggle_content_filter(&mut self, target: ContentFilter) {
        if self.content_filter == target {
            self.content_filter = ContentFilter::All;
        } else {
            self.content_filter = target;
        }
    }

    pub(crate) const fn toggle_case_sensitive_search(&mut self) {
        self.search_options.case_sensitive = !self.search_options.case_sensitive;
    }

    pub(crate) const fn cycle_search_match_mode(&mut self) {
        self.search_options.match_mode = self.search_options.match_mode.next();
    }

    /// Get filtered records based on search query and content type filter
    fn get_filtered_records(&self, query: &str) -> Vec<ClipboardRecord> {
        let records = self.records.lock().unwrap_or_else(PoisonError::into_inner);

        let filtered =
            filter_records_by_query(&records, query, self.content_filter, self.search_options);

        drop(records); // Release the lock early

        let mut sorted_records = filtered;
        ClipboardRepository::sort_pinned_first(&mut sorted_records);
        sorted_records
    }

    /// Confirm selection: copy record to clipboard and hide.
    /// The clipboard listener will re-capture the copy event and the
    /// repository layer handles deduplication via content hash upsert.
    pub(crate) fn confirm_record(&self, window: &mut Window, cx: &Context<Self>, index: usize) {
        let (content, content_type) = {
            if let Some(record) = self.filtered_records.get(index) {
                (record.content.clone(), record.content_type.clone())
            } else {
                return;
            }
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
            let new_filtered_records = self.get_filtered_records(&query);

            if new_filtered_records != *self.filtered_records {
                let old_len = self.filtered_records.len();
                let new_len = new_filtered_records.len();

                // If we're deleting a record, preserve the scroll position
                let scroll_position = if self.deleting_record {
                    Some(self.list_state.logical_scroll_top())
                } else {
                    None
                };

                self.filtered_records = Arc::new(new_filtered_records);

                // Use splice to inform list state about the change instead of reset
                // This helps preserve scroll position better
                if old_len > new_len && self.deleting_record {
                    // A record was deleted - use splice to update just that range
                    // We replace the entire range with the new count
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

            if self.selected_index >= self.filtered_records.len()
                && !self.filtered_records.is_empty()
            {
                self.selected_index = self.filtered_records.len() - 1;
            } else if self.filtered_records.is_empty() {
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
                this.child(clear_confirm::render_clear_confirm_overlay(self, cx))
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
    use super::*;
    use crate::config::ConfirmMode;

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

        // Simulate toggle to Text while Image is active -> switches to Text
        filter = if filter == ContentFilter::Text {
            ContentFilter::All
        } else {
            ContentFilter::Text
        };
        assert_eq!(filter, ContentFilter::Text);
    }
}

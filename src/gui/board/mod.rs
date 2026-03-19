mod actions;
mod preview;
mod records_list;
mod render;

// moved panels to gui::panel
use std::{
    borrow::Cow,
    str::FromStr,
    sync::{Arc, Mutex, PoisonError, RwLock, mpsc},
    time::Duration,
};

// Re-export utilities for external use
pub use actions::{Active, ConfirmSelection, Hide, Quit, SelectNext, SelectPrev};
use gpui::{
    AnyElement, AppContext, Context, Entity, FocusHandle, Focusable, ListAlignment, ListState,
    Render, SharedString, Subscription, Window,
    prelude::{FluentBuilder, InteractiveElement, IntoElement, ParentElement, Styled},
    px,
};
use gpui_component::{
    ActiveTheme, IndexPath, WindowExt,
    input::InputState,
    notification::Notification,
    select::{SelectEvent, SelectState},
    v_flex,
};
use render::{render_header, render_search_input};

use crate::{
    clipboard::LastCopyState,
    config::{ConfirmMode, Settings},
    gui::{
        hide_window, hotkey_record,
        panel::{
            about::render_about_content, help::render_help_content,
            settings::render_settings_content,
        },
    },
    i18n::{I18n, Language},
    repository::{ClipboardRecord, ClipboardRepository, models::ContentType},
    updater::models::UpdateStatus,
};

/// Content type filter for the clipboard history view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentFilter {
    /// Show all content types
    #[default]
    All,
    /// Show only text records
    Text,
    /// Show only image records
    Image,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMatchMode {
    #[default]
    Contains,
    WholeWord,
    Exact,
}

impl SearchMatchMode {
    pub(crate) const fn next(self) -> Self {
        match self {
            Self::Contains => Self::WholeWord,
            Self::WholeWord => Self::Exact,
            Self::Exact => Self::Contains,
        }
    }

    pub(crate) const fn short_label(self) -> &'static str {
        match self {
            Self::Contains => ".*",
            Self::WholeWord => "W",
            Self::Exact => "=",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchOptions {
    pub(crate) match_mode: SearchMatchMode,
    pub(crate) case_sensitive: bool,
}

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
    // Settings
    pub(crate) settings: Arc<RwLock<Settings>>,
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

    pub(crate) fn displayed_hotkey(&self) -> &str {
        &self.pending_hotkey
    }

    pub(crate) fn start_hotkey_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hotkey_before_recording = self.pending_hotkey.clone();
        self.hotkey_recording = true;
        self.hotkey_manual_editing = false;
        self.settings_activation_key_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn enable_hotkey_manual_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.hotkey_recording = false;
        self.hotkey_manual_editing = true;
        let pending_hotkey = self.pending_hotkey.clone();
        self.settings_activation_key_input.update(cx, |input, cx| {
            input.set_value(pending_hotkey, window, cx);
        });
        window.focus(&self.settings_activation_key_input.focus_handle(cx));
        cx.notify();
    }

    pub(crate) fn clear_hotkey_candidate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hotkey_recording = false;
        self.pending_hotkey.clear();
        self.settings_activation_key_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn cancel_hotkey_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hotkey_recording = false;
        self.pending_hotkey
            .clone_from(&self.hotkey_before_recording);
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn on_settings_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.hotkey_recording {
            return;
        }

        if hotkey_record::is_cancel_key(&event.keystroke.key) {
            self.cancel_hotkey_recording(window, cx);
            return;
        }

        if hotkey_record::is_clear_key(&event.keystroke) {
            self.clear_hotkey_candidate(window, cx);
            return;
        }

        let Some(hotkey) = hotkey_record::keystroke_to_hotkey(&event.keystroke) else {
            return;
        };

        self.hotkey_recording = false;
        self.hotkey_manual_editing = false;
        self.pending_hotkey = hotkey;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn resolve_activation_key_input(&self, cx: &Context<Self>) -> String {
        if self.hotkey_manual_editing {
            self.settings_activation_key_input
                .read(cx)
                .value()
                .trim()
                .to_string()
        } else {
            self.pending_hotkey.trim().to_string()
        }
    }

    #[allow(clippy::too_many_lines)]
    pub fn new(
        records: Arc<Mutex<Vec<ClipboardRecord>>>,
        repository: Option<Arc<ClipboardRepository>>,
        settings: Arc<RwLock<Settings>>,
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
                    hide_window(window, cx, this.pinned);
                }
            });

        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Use / to search ... "));
        // Measure all items initially so scrollbar thumb size is stable on first paint.
        let list_state = ListState::new(0, ListAlignment::Top, gpui::px(100.)).measure_all();

        let (
            max_history_records,
            max_storage_records,
            activation_key,
            theme_index,
            language,
            confirm_mode,
        ) = {
            let settings_guard = match settings.read() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
            let theme_idx = match settings_guard.theme {
                crate::config::AppTheme::Light => 0,
                crate::config::AppTheme::Dark => 1,
                crate::config::AppTheme::System => 2,
            };
            (
                settings_guard.storage.max_history_records,
                settings_guard.storage.max_storage_records,
                settings_guard.hotkey.activation_key.clone(),
                theme_idx,
                settings_guard.language.clone(),
                settings_guard.confirm.mode,
            )
        };
        let autostart_enabled = match settings.read() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
        .autostart
        .enabled;
        let auto_check_enabled = match settings.read() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
        .update
        .auto_check;
        let hover_preview_enabled = match settings.read() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
        .preview
        .hover_preview_enabled;
        let settings_activation_key_input = cx.new(|cx| InputState::new(window, cx));
        let settings_max_history_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(max_history_records.to_string()));
        let settings_max_storage_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(max_storage_records.to_string()));

        // Initialize I18n with the language from settings
        let i18n = I18n::new(language.clone()).unwrap_or_default();
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

        Self {
            records,
            repository,
            settings,
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
    fn clear_history(&self) {
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
    fn clear_last_copy_state(&self) {
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

    fn is_token_char(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    fn has_word_boundaries(content: &str, start: usize, end: usize) -> bool {
        let previous = content[..start].chars().next_back();
        let next = content[end..].chars().next();

        let has_left_boundary = previous.is_none_or(|ch| !Self::is_token_char(ch));
        let has_right_boundary = next.is_none_or(|ch| !Self::is_token_char(ch));

        has_left_boundary && has_right_boundary
    }

    fn normalized_text(text: &str, case_sensitive: bool) -> Cow<'_, str> {
        if case_sensitive {
            Cow::Borrowed(text)
        } else {
            Cow::Owned(text.to_lowercase())
        }
    }

    fn text_matches_query(content: &str, query: &str, options: SearchOptions) -> bool {
        if query.is_empty() {
            return true;
        }

        let normalized_content = Self::normalized_text(content, options.case_sensitive);
        let normalized_query = Self::normalized_text(query, options.case_sensitive);

        match options.match_mode {
            SearchMatchMode::Contains => normalized_content.contains(normalized_query.as_ref()),
            SearchMatchMode::WholeWord => normalized_content
                .match_indices(normalized_query.as_ref())
                .any(|(start, matched)| {
                    let end = start + matched.len();
                    Self::has_word_boundaries(normalized_content.as_ref(), start, end)
                }),
            SearchMatchMode::Exact => normalized_content == normalized_query,
        }
    }

    /// Filter records based on search query and content type filter
    fn filter_records_by_query(
        records: &[ClipboardRecord],
        query: &str,
        filter: ContentFilter,
        options: SearchOptions,
    ) -> Vec<ClipboardRecord> {
        records
            .iter()
            .filter(|record| {
                // Apply content type filter
                let passes_type_filter = match filter {
                    ContentFilter::All => true,
                    ContentFilter::Text => record.content_type == ContentType::Text,
                    ContentFilter::Image => record.content_type == ContentType::Image,
                };

                if !passes_type_filter {
                    return false;
                }

                // Image filter: ignore query entirely (images cannot be text-searched)
                if filter == ContentFilter::Image {
                    return true;
                }

                // Text/All filter: apply text search on text records
                if query.is_empty() {
                    return true;
                }

                record.content_type == ContentType::Text
                    && Self::text_matches_query(&record.content, query, options)
            })
            .cloned()
            .collect()
    }

    /// Get filtered records based on search query and content type filter
    fn get_filtered_records(&self, query: &str) -> Vec<ClipboardRecord> {
        let records = self.records.lock().unwrap_or_else(PoisonError::into_inner);

        let filtered = Self::filter_records_by_query(
            &records,
            query,
            self.content_filter,
            self.search_options,
        );

        drop(records); // Release the lock early

        let mut sorted_records = filtered;
        ClipboardRepository::sort_pinned_first(&mut sorted_records);
        sorted_records
    }

    /// Confirm selection: copy record to clipboard and hide.
    /// The clipboard listener will re-capture the copy event and the
    /// repository layer handles deduplication via content hash upsert.
    fn confirm_record(&self, window: &mut Window, cx: &Context<Self>, index: usize) {
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

    #[allow(clippy::too_many_lines)]
    pub(crate) fn save_settings(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        self.hotkey_recording = false;
        let mut activation_key = self.resolve_activation_key_input(cx);

        let mut is_hotkey_invalid = false;
        if activation_key.is_empty() {
            activation_key.clone_from(
                &match self.settings.read() {
                    Ok(g) => g,
                    Err(e) => e.into_inner(),
                }
                .hotkey
                .activation_key,
            );
            // If current setting is also empty (should not happen with load fix), use default
            if activation_key.is_empty() {
                activation_key = Settings::default().hotkey.activation_key;
            }
        } else if global_hotkey::hotkey::HotKey::from_str(&activation_key).is_err() {
            is_hotkey_invalid = true;
            activation_key = Settings::default().hotkey.activation_key;
        }

        self.pending_hotkey.clone_from(&activation_key);
        self.hotkey_before_recording.clone_from(&activation_key);
        self.hotkey_manual_editing = false;

        // Get current values from settings as fallback
        let (current_max_history, current_max_storage) = {
            let settings_guard = match self.settings.read() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
            (
                settings_guard.storage.max_history_records,
                settings_guard.storage.max_storage_records,
            )
        };

        // Validate max_history input from the settings UI.
        let max_history_input = self.settings_max_history_input.read(cx).value().to_string();

        let (max_history, is_max_history_invalid) =
            match Self::parse_max_history_input(&max_history_input, current_max_history) {
                Ok(v) => (v, false),
                Err(()) => (current_max_history, true),
            };

        // Validate max_storage input from the settings UI.
        let max_storage_input = self.settings_max_storage_input.read(cx).value().to_string();

        let (mut max_storage, is_max_storage_invalid) =
            match Self::parse_max_storage_input(&max_storage_input, current_max_storage) {
                Ok(v) => (v, false),
                Err(()) => (current_max_storage, true),
            };

        // Ensure max_storage >= max_history
        let is_max_storage_lt_history = max_storage < max_history;
        if is_max_storage_lt_history {
            max_storage = max_history;
        }

        let theme = match self.selected_theme {
            0 => crate::config::AppTheme::Light,
            1 => crate::config::AppTheme::Dark,
            _ => crate::config::AppTheme::System,
        };

        let language = Language::all()
            .get(self.selected_language)
            .cloned()
            .unwrap_or_default();

        let mut save_disk_error: Option<String> = None;
        {
            let mut settings = match self.settings.write() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
            settings.hotkey.activation_key.clone_from(&activation_key);
            settings.storage.max_history_records = max_history;
            settings.storage.max_storage_records = max_storage;
            settings.theme = theme.clone();
            settings.autostart.enabled = self.autostart_enabled;
            settings.language = language.clone();
            settings.update.auto_check = self.auto_check_enabled;
            settings.preview.hover_preview_enabled = self.hover_preview_enabled;
            settings.confirm.mode = self.confirm_mode;
            if let Err(e) = settings.save() {
                tracing::warn!(error = %e, "failed to save settings");
                save_disk_error = Some(e.to_string());
            }
        }

        // Update hotkey if sender is available
        if let Some(tx) = &self.hotkey_tx {
            let _ = tx.try_send(activation_key.clone());
        }

        // Apply the new language
        if let Err(e) = self.i18n.set_language(language) {
            tracing::warn!(error = ?e, "failed to set language");
        }

        // Update search placeholder with new language
        let search_placeholder = self.i18n.t("search_placeholder");
        self.search_input.update(cx, |input, cx| {
            input.set_placeholder(search_placeholder, window, cx);
        });

        // Sync auto-start state with system
        let autostart_error = self.sync_autostart_state().err();
        if let Some(ref e) = autostart_error {
            tracing::warn!(error = ?e, "failed to sync auto-start state");
        }

        // Apply the new theme
        let app_theme = &theme.get_theme();
        crate::gui::app::set_app_theme(window, cx, app_theme);

        self.settings_max_history_input.update(cx, |input, cx| {
            input.set_placeholder(max_history.to_string(), window, cx);
            input.set_value("", window, cx);
        });

        self.settings_max_storage_input.update(cx, |input, cx| {
            input.set_placeholder(max_storage.to_string(), window, cx);
            input.set_value("", window, cx);
        });

        // --- User notifications: auto width (content-driven), capped at 280px ---
        if let Some(err_msg) = save_disk_error {
            let msg = format!("✕  {}: {}", self.i18n.t("settings_save_failed"), err_msg);
            window.push_notification(
                Notification::new().message(msg).w_auto().max_w(px(280.0)),
                cx,
            );
        } else {
            if is_hotkey_invalid {
                let warn_msg = self.i18n.t("settings_hotkey_invalid_warning");
                window.push_notification(
                    Notification::new()
                        .message(format!("⚠  {warn_msg}"))
                        .w_auto()
                        .max_w(px(280.0)),
                    cx,
                );
            }
            if is_max_history_invalid {
                let warn_msg = self.i18n.t("settings_max_history_invalid_warning");
                window.push_notification(
                    Notification::new()
                        .message(format!("⚠  {warn_msg}"))
                        .w_auto()
                        .max_w(px(280.0)),
                    cx,
                );
            }
            if is_max_storage_invalid {
                let warn_msg = self.i18n.t("settings_max_storage_invalid_warning");
                window.push_notification(
                    Notification::new()
                        .message(format!("⚠  {warn_msg}"))
                        .w_auto()
                        .max_w(px(280.0)),
                    cx,
                );
            }
            if is_max_storage_lt_history {
                let warn_msg = self.i18n.t("settings_max_storage_lt_history_warning");
                window.push_notification(
                    Notification::new()
                        .message(format!("⚠  {warn_msg}"))
                        .w_auto()
                        .max_w(px(280.0)),
                    cx,
                );
            }
            if autostart_error.is_some() {
                let warn_msg = self.i18n.t("settings_autostart_failed");
                window.push_notification(
                    Notification::new()
                        .message(format!("⚠  {warn_msg}"))
                        .w_auto()
                        .max_w(px(280.0)),
                    cx,
                );
            }
            if !is_hotkey_invalid
                && autostart_error.is_none()
                && !is_max_history_invalid
                && !is_max_storage_invalid
                && !is_max_storage_lt_history
            {
                let ok_msg = self.i18n.t("settings_save_success");
                window.push_notification(
                    Notification::new()
                        .message(format!("✓  {ok_msg}"))
                        .w_auto()
                        .max_w(px(280.0)),
                    cx,
                );
            }
        }

        cx.notify();
    }

    pub(crate) fn toggle_autostart(&mut self, cx: &mut Context<Self>) {
        self.autostart_enabled = !self.autostart_enabled;
        cx.notify();
    }

    fn sync_autostart_state(&self) -> Result<(), crate::config::AutoStartError> {
        use crate::constants::APP_NAME;
        let manager = crate::config::AutoStartManager::new(APP_NAME)?;
        manager.sync_state(self.autostart_enabled)?;
        Ok(())
    }

    // Validate max history input from settings UI.
    // Returns Ok(parsed_value) when input is a valid usize within allowed range,
    // or Err(()) when input is invalid (parse error or out of range).
    fn parse_max_history_input(input: &str, current_max: usize) -> Result<usize, ()> {
        const MIN: usize = 1;
        const MAX: usize = 10_000;

        let s = input.trim();
        if s.is_empty() {
            return Ok(current_max);
        }

        match s.parse::<usize>() {
            Ok(v) if (MIN..=MAX).contains(&v) => Ok(v),
            _ => Err(()),
        }
    }

    // Validate max storage input from settings UI.
    // Returns Ok(parsed_value) when input is a valid usize within allowed range,
    // or Err(()) when input is invalid (parse error or out of range).
    fn parse_max_storage_input(input: &str, current_max: usize) -> Result<usize, ()> {
        const MIN: usize = 1;
        const MAX: usize = 100_000;

        let s = input.trim();
        if s.is_empty() {
            return Ok(current_max);
        }

        match s.parse::<usize>() {
            Ok(v) if (MIN..=MAX).contains(&v) => Ok(v),
            _ => Err(()),
        }
    }

    /// Trigger a manual update check in the background
    pub fn check_for_update_async(&mut self, cx: &mut Context<Self>) {
        self.update_status = UpdateStatus::Checking;
        cx.notify();

        let include_prerelease = match self.settings.read() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
        .update
        .include_prerelease;

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    // Use std::thread::spawn to run blocking operation
                    let (tx, rx) = std::sync::mpsc::channel();
                    let _handle = std::thread::spawn(move || {
                        let update_result =
                            crate::updater::checker::check_for_update(include_prerelease);
                        let _ = tx.send(update_result);
                    });

                    rx.recv().unwrap_or_else(|_| {
                        Err(crate::updater::errors::UpdateError::Network(
                            "Update check failed".to_string(),
                        ))
                    })
                })
                .await;

            let _ = this.update(cx, |board, cx| {
                match result {
                    Ok(Some(info)) => {
                        board.update_status = UpdateStatus::Available(info);
                    }
                    Ok(None) => {
                        board.update_status = UpdateStatus::UpToDate;
                    }
                    Err(e) => {
                        tracing::warn!(error = ?e, "update check failed");
                        board.update_status = UpdateStatus::Error(e.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Trigger download and install in the background
    pub fn download_and_install_update(&mut self, cx: &mut Context<Self>) {
        let release = match &self.update_status {
            UpdateStatus::Available(info) => info.clone(),
            _ => return,
        };
        self.update_status = UpdateStatus::Downloading(0.0);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    // Use std::thread::spawn to run blocking operation
                    let (tx, rx) = std::sync::mpsc::channel();
                    let _handle = std::thread::spawn(move || {
                        let update_result = crate::updater::downloader::download_and_install(
                            &release,
                            |_progress| {
                                // Progress callback runs on blocking thread;
                                // mid-download UI updates are skipped for simplicity.
                            },
                        );
                        let _ = tx.send(update_result);
                    });

                    rx.recv().unwrap_or_else(|_| {
                        Err(crate::updater::errors::UpdateError::Network(
                            "Update installation failed".to_string(),
                        ))
                    })
                })
                .await;

            let _ = this.update(cx, |board, cx| {
                match result {
                    Ok(()) => {
                        board.update_status = UpdateStatus::ReadyToRestart;
                    }
                    Err(e) => {
                        tracing::error!(error = ?e, "update installation failed");
                        board.update_status = UpdateStatus::Error(e.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
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
                this.child(render::render_clear_confirm_overlay(self, cx))
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
    use crate::repository::{ClipboardRecord, models::ContentType};

    /// Helper: build a mixed set of test records (2 text + 1 image)
    fn mixed_records() -> Vec<ClipboardRecord> {
        vec![
            ClipboardRecord {
                id: 1,
                content: "Hello World".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "Goodbye World".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 3,
                content: "image_data".to_string(),
                content_type: ContentType::Image,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ]
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

    // --- ContentFilter::All (default) tests ---

    #[test]
    fn test_filter_all_no_query_returns_everything() {
        let records = mixed_records();
        let result = RopyBoard::filter_records_by_query(
            &records,
            "",
            ContentFilter::All,
            SearchOptions::default(),
        );
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_filter_all_with_query_matches_text_only() {
        let records = mixed_records();
        let result = RopyBoard::filter_records_by_query(
            &records,
            "world",
            ContentFilter::All,
            SearchOptions::default(),
        );
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "Hello World");
        assert_eq!(result[1].content, "Goodbye World");
    }

    #[test]
    fn test_search_contains_case_insensitive_matches_all_variants() {
        let records = vec![
            ClipboardRecord {
                id: 1,
                content: "Hello World".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "HELLO world".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ];

        let result = RopyBoard::filter_records_by_query(
            &records,
            "hello",
            ContentFilter::All,
            SearchOptions::default(),
        );
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_search_contains_case_sensitive_matches_only_same_case() {
        let records = vec![
            ClipboardRecord {
                id: 1,
                content: "Hello World".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "hello world".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ];

        let result = RopyBoard::filter_records_by_query(
            &records,
            "Hello",
            ContentFilter::All,
            SearchOptions {
                match_mode: SearchMatchMode::Contains,
                case_sensitive: true,
            },
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "Hello World");
    }

    #[test]
    fn test_search_whole_word_case_insensitive_matches_token_boundaries() {
        let records = vec![
            ClipboardRecord {
                id: 1,
                content: "Say hello world".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "say HELLO again".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 3,
                content: "shelloworld".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ];

        let result = RopyBoard::filter_records_by_query(
            &records,
            "hello",
            ContentFilter::All,
            SearchOptions {
                match_mode: SearchMatchMode::WholeWord,
                case_sensitive: false,
            },
        );

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_search_whole_word_case_sensitive_rejects_case_mismatch() {
        let records = vec![
            ClipboardRecord {
                id: 1,
                content: "say Hello again".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "say hello again".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ];

        let result = RopyBoard::filter_records_by_query(
            &records,
            "Hello",
            ContentFilter::All,
            SearchOptions {
                match_mode: SearchMatchMode::WholeWord,
                case_sensitive: true,
            },
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "say Hello again");
    }

    #[test]
    fn test_search_whole_word_partial_token_returns_no_match() {
        let records = vec![ClipboardRecord {
            id: 1,
            content: "hello_world hello2".to_string(),
            content_type: ContentType::Text,
            created_at: chrono::Local::now(),
            pinned: false,
        }];

        let result = RopyBoard::filter_records_by_query(
            &records,
            "hello",
            ContentFilter::All,
            SearchOptions {
                match_mode: SearchMatchMode::WholeWord,
                case_sensitive: false,
            },
        );

        assert!(result.is_empty());
    }

    #[test]
    fn test_search_exact_case_insensitive_matches_full_content() {
        let records = vec![
            ClipboardRecord {
                id: 1,
                content: "Hello".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "HELLO".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 3,
                content: "Hello World".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ];

        let result = RopyBoard::filter_records_by_query(
            &records,
            "hello",
            ContentFilter::All,
            SearchOptions {
                match_mode: SearchMatchMode::Exact,
                case_sensitive: false,
            },
        );

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_search_exact_case_sensitive_matches_only_strict_equal_content() {
        let records = vec![
            ClipboardRecord {
                id: 1,
                content: "Hello".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "hello".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 3,
                content: " hello ".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ];

        let result = RopyBoard::filter_records_by_query(
            &records,
            "hello",
            ContentFilter::All,
            SearchOptions {
                match_mode: SearchMatchMode::Exact,
                case_sensitive: true,
            },
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "hello");
    }

    #[test]
    fn test_filter_all_with_query_no_matches() {
        let records = vec![ClipboardRecord {
            id: 1,
            content: "Hello".to_string(),
            content_type: ContentType::Text,
            created_at: chrono::Local::now(),
            pinned: false,
        }];

        let result = RopyBoard::filter_records_by_query(
            &records,
            "xyz",
            ContentFilter::All,
            SearchOptions::default(),
        );
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_filter_all_with_query_excludes_image() {
        let records = vec![ClipboardRecord {
            id: 1,
            content: "Image content".to_string(),
            content_type: ContentType::Image,
            created_at: chrono::Local::now(),
            pinned: false,
        }];

        let result = RopyBoard::filter_records_by_query(
            &records,
            "image",
            ContentFilter::All,
            SearchOptions::default(),
        );
        assert_eq!(result.len(), 0);
    }

    // --- ContentFilter::Text tests ---

    #[test]
    fn test_filter_text_no_query_returns_text_only() {
        let records = mixed_records();
        let result = RopyBoard::filter_records_by_query(
            &records,
            "",
            ContentFilter::Text,
            SearchOptions::default(),
        );
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|r| r.content_type == ContentType::Text));
    }

    #[test]
    fn test_filter_text_with_query_matches_within_text() {
        let records = mixed_records();
        let result = RopyBoard::filter_records_by_query(
            &records,
            "hello",
            ContentFilter::Text,
            SearchOptions::default(),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "Hello World");
    }

    // --- ContentFilter::Image tests ---

    #[test]
    fn test_filter_image_no_query_returns_images_only() {
        let records = mixed_records();
        let result = RopyBoard::filter_records_by_query(
            &records,
            "",
            ContentFilter::Image,
            SearchOptions::default(),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content_type, ContentType::Image);
    }

    #[test]
    fn test_filter_image_with_query_ignores_query_for_all_search_modes() {
        let records = mixed_records();
        for options in [
            SearchOptions::default(),
            SearchOptions {
                match_mode: SearchMatchMode::WholeWord,
                case_sensitive: false,
            },
            SearchOptions {
                match_mode: SearchMatchMode::Exact,
                case_sensitive: true,
            },
        ] {
            let result = RopyBoard::filter_records_by_query(
                &records,
                "nonexistent",
                ContentFilter::Image,
                options,
            );
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].content_type, ContentType::Image);
        }
    }

    // --- Toggle tests ---

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

        // Simulate toggle Text again → back to All
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

        // Simulate toggle to Text while Image is active → switches to Text
        filter = if filter == ContentFilter::Text {
            ContentFilter::All
        } else {
            ContentFilter::Text
        };
        assert_eq!(filter, ContentFilter::Text);
    }

    #[test]
    fn test_search_match_mode_next_cycles_all_modes() {
        assert_eq!(SearchMatchMode::Contains.next(), SearchMatchMode::WholeWord);
        assert_eq!(SearchMatchMode::WholeWord.next(), SearchMatchMode::Exact);
        assert_eq!(SearchMatchMode::Exact.next(), SearchMatchMode::Contains);
    }

    // --- Existing parse tests (unchanged) ---

    #[test]
    fn test_parse_max_history_input_empty_uses_current() {
        let current = 42usize;
        let res = RopyBoard::parse_max_history_input("", current);
        assert_eq!(res, Ok(current));
    }

    #[test]
    fn test_parse_max_history_input_valid() {
        let current = 10usize;
        let res = RopyBoard::parse_max_history_input("100", current);
        assert_eq!(res, Ok(100usize));
    }

    #[test]
    fn test_parse_max_history_input_invalid_string() {
        let current = 10usize;
        let res = RopyBoard::parse_max_history_input("abc", current);
        assert!(res.is_err());
    }

    #[test]
    fn test_parse_max_history_input_out_of_range() {
        let current = 10usize;
        // zero is below minimum
        assert!(RopyBoard::parse_max_history_input("0", current).is_err());
        // above maximum (10_000)
        assert!(RopyBoard::parse_max_history_input("10001", current).is_err());
    }
}

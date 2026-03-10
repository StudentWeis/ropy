mod actions;
mod preview;
mod render;

// moved panels to gui::panel
use std::{
    str::FromStr,
    sync::{Arc, Mutex, PoisonError, RwLock},
};

// Re-export utilities for external use
pub use actions::{Active, ConfirmSelection, DeleteRecord, Hide, Quit, SelectNext, SelectPrev};
use gpui::{
    AppContext, Context, Entity, FocusHandle, ListAlignment, ListState, Render, SharedString,
    Subscription, Window,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
};
use gpui_component::{
    ActiveTheme, IndexPath,
    input::InputState,
    select::{SelectEvent, SelectState},
    v_flex,
};
use render::{render_header, render_search_input};

use crate::{
    clipboard::LastCopyState,
    config::Settings,
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
    // Settings
    pub(crate) settings: Arc<RwLock<Settings>>,
    pub(crate) show_settings: bool,
    pub(crate) show_about: bool,
    pub(crate) show_help: bool,
    pub(crate) show_preview: bool,
    pub(crate) settings_activation_key_input: Entity<InputState>,
    pub(crate) settings_max_history_input: Entity<InputState>,
    pub(crate) selected_theme: usize, // 0: Light, 1: Dark, 2: System
    pub(crate) autostart_enabled: bool,
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
}

impl RopyBoard {
    pub fn set_hotkey_tx(&mut self, tx: async_channel::Sender<String>) {
        self.hotkey_tx = Some(tx);
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

        let (max_history_records, activation_key, theme_index, language) = {
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
                settings_guard.hotkey.activation_key.clone(),
                theme_idx,
                settings_guard.language.clone(),
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
        let settings_activation_key_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(activation_key.clone()));
        let settings_max_history_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(max_history_records.to_string()));

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
            settings_activation_key_input,
            settings_max_history_input,
            selected_theme: theme_index,
            autostart_enabled,
            pinned: false,
            hotkey_tx: None,
            i18n,
            selected_language,
            language_select,
            deleting_record: false,
            update_status: UpdateStatus::Idle,
            auto_check_enabled,
            hover_preview_enabled,
        }
    }

    /// Copy content to clipboard
    fn copy_to_clipboard(&self, content: &str, content_type: &ContentType) {
        let request = match content_type {
            ContentType::Text => Some(crate::clipboard::CopyRequest::Text(content.to_string())),
            ContentType::Image => Some(crate::clipboard::CopyRequest::Image(content.to_string())),
            ContentType::FilePath => todo!(),
        };

        if let Some(req) = request {
            let _ = self.copy_tx.send_blocking(req);
        }
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

    /// Filter records based on search query
    fn filter_records_by_query(records: &[ClipboardRecord], query: &str) -> Vec<ClipboardRecord> {
        if query.is_empty() {
            return records.to_vec();
        }

        let query_lower = query.to_lowercase();
        records
            .iter()
            .filter(|record| {
                record.content_type == ContentType::Text
                    && record.content.to_lowercase().contains(&query_lower)
            })
            .cloned()
            .collect()
    }

    /// Get filtered records based on search query
    fn get_filtered_records(&self, query: &str) -> Vec<ClipboardRecord> {
        let records = self.records.lock().unwrap_or_else(PoisonError::into_inner);

        let filtered = Self::filter_records_by_query(&records, query);

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
        self.copy_to_clipboard(&content, &content_type);
        if !self.pinned {
            hide_window(window, cx, self.pinned);
        }
    }

    pub(crate) fn save_settings(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        let mut activation_key = self
            .settings_activation_key_input
            .read(cx)
            .value()
            .to_string();

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

        // Get current max_history_records from settings as fallback
        let current_max_history = match self.settings.read() {
            Ok(g) => g,
            Err(e) => e.into_inner(),
        }
        .storage
        .max_history_records;

        let max_history = self
            .settings_max_history_input
            .read(cx)
            .value()
            .to_string()
            .parse::<usize>()
            .unwrap_or(current_max_history);

        let theme = match self.selected_theme {
            0 => crate::config::AppTheme::Light,
            1 => crate::config::AppTheme::Dark,
            _ => crate::config::AppTheme::System,
        };

        let language = Language::all()
            .get(self.selected_language)
            .cloned()
            .unwrap_or_default();

        {
            let mut settings = match self.settings.write() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            };
            settings.hotkey.activation_key.clone_from(&activation_key);
            settings.storage.max_history_records = max_history;
            settings.theme = theme.clone();
            settings.autostart.enabled = self.autostart_enabled;
            settings.language = language.clone();
            settings.update.auto_check = self.auto_check_enabled;
            settings.preview.hover_preview_enabled = self.hover_preview_enabled;
            if let Err(e) = settings.save() {
                tracing::warn!(error = %e, "failed to save settings");
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
        if let Err(e) = self.sync_autostart_state() {
            tracing::warn!(error = ?e, "failed to sync auto-start state");
        }

        // Apply the new theme
        let app_theme = &theme.get_theme();
        crate::gui::app::set_app_theme(window, cx, app_theme);

        self.settings_max_history_input.update(cx, |input, cx| {
            input.set_placeholder(max_history.to_string(), window, cx);
            input.set_value("", window, cx);
        });

        let hotkey_invalid_msg = self.i18n.t("settings_hotkey_invalid");
        self.settings_activation_key_input.update(cx, |input, cx| {
            input.set_placeholder(activation_key, window, cx);
            if is_hotkey_invalid {
                input.set_value(hotkey_invalid_msg, window, cx);
            } else {
                input.set_value("", window, cx);
            }
        });
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
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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

        if self.show_settings {
            return base.child(render_settings_content(self, cx));
        }

        if self.show_about {
            return base.child(render_about_content(self, cx));
        }

        if self.show_help {
            return base.child(render_help_content(self, cx));
        }

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

        if self.selected_index >= self.filtered_records.len() && !self.filtered_records.is_empty() {
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
            .child(render_search_input(&self.search_input, cx))
            .child(self.render_records_list(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::{ClipboardRecord, models::ContentType};

    #[test]
    fn test_filter_records_by_query_empty_query() {
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
                content: "World".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 3,
                content: "Image data".to_string(),
                content_type: ContentType::Image,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ];

        let result = RopyBoard::filter_records_by_query(&records, "");
        assert_eq!(result.len(), 3); // With empty query, all records should be returned
        assert_eq!(result[0].content, "Hello");
        assert_eq!(result[1].content, "World");
        assert_eq!(result[2].content, "Image data");
    }

    #[test]
    fn test_filter_records_by_query_with_matches() {
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
                content: "Goodbye World".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 3,
                content: "Image data".to_string(),
                content_type: ContentType::Image,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ];

        let result = RopyBoard::filter_records_by_query(&records, "world");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "Hello World");
        assert_eq!(result[1].content, "Goodbye World");
    }

    #[test]
    fn test_filter_records_by_query_case_insensitive() {
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

        let result = RopyBoard::filter_records_by_query(&records, "hello");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "Hello World");
        assert_eq!(result[1].content, "HELLO world");
    }

    #[test]
    fn test_filter_records_by_query_no_matches() {
        let records = vec![ClipboardRecord {
            id: 1,
            content: "Hello".to_string(),
            content_type: ContentType::Text,
            created_at: chrono::Local::now(),
            pinned: false,
        }];

        let result = RopyBoard::filter_records_by_query(&records, "xyz");
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_filter_records_by_query_non_text_content_type() {
        let records = vec![ClipboardRecord {
            id: 1,
            content: "Image content".to_string(),
            content_type: ContentType::Image,
            created_at: chrono::Local::now(),
            pinned: false,
        }];

        let result = RopyBoard::filter_records_by_query(&records, "image");
        assert_eq!(result.len(), 0); // Image content should not match even if content contains the query
    }
}

mod actions;
mod render;
mod settings;

use crate::config::Settings;
use crate::gui::hide_window;
use crate::repository::models::ContentType;
use crate::repository::{ClipboardRecord, ClipboardRepository};
use gpui::{
    AppContext, Context, Entity, FocusHandle, ListAlignment, ListState, Render, Subscription,
    Window,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
};
use gpui_component::input::InputState;
use gpui_component::{ActiveTheme, v_flex};
use std::sync::{Arc, Mutex, RwLock, mpsc};

// Re-export utilities for external use
pub use actions::{Active, ConfirmSelection, Hide, Quit, SelectNext, SelectPrev};
use render::{render_header, render_search_input};
use settings::render_settings_content;

/// RopyBoard Main Window Component
pub struct RopyBoard {
    records: Arc<Mutex<Vec<ClipboardRecord>>>,
    repository: Option<Arc<ClipboardRepository>>,
    settings: Arc<RwLock<Settings>>,
    focus_handle: FocusHandle,
    _focus_out_subscription: Subscription,
    search_input: Entity<InputState>,
    selected_index: usize,
    list_state: ListState,
    filtered_records: Vec<ClipboardRecord>,
    copy_tx: mpsc::Sender<crate::clipboard::CopyRequest>,

    // Settings
    show_settings: bool,
    settings_activation_key_input: Entity<InputState>,
    settings_max_history_input: Entity<InputState>,
    selected_theme: usize, // 0: Light, 1: Dark, 2: System
    autostart_enabled: bool,
}

impl RopyBoard {
    pub fn new(
        records: Arc<Mutex<Vec<ClipboardRecord>>>,
        repository: Option<Arc<ClipboardRepository>>,
        settings: Arc<RwLock<Settings>>,
        copy_tx: mpsc::Sender<crate::clipboard::CopyRequest>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);

        // Subscribe to focus out events to hide the window
        let _focus_out_subscription =
            cx.on_focus_out(&focus_handle, window, move |_this, _event, window, cx| {
                // When the window loses focus, hide the window
                hide_window(window, cx);
            });

        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search ... "));
        let list_state = ListState::new(0, ListAlignment::Top, gpui::px(100.));

        let (max_history_records, activation_key, theme_index) = {
            let settings_guard = settings.read().unwrap();
            let theme_idx = match settings_guard.theme {
                crate::config::AppTheme::Light => 0,
                crate::config::AppTheme::Dark => 1,
                crate::config::AppTheme::System => 2,
            };
            (
                settings_guard.storage.max_history_records,
                settings_guard.hotkey.activation_key.clone(),
                theme_idx,
            )
        };
        let autostart_enabled = settings.read().unwrap().autostart.enabled;
        let settings_activation_key_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(activation_key.to_string()));
        let settings_max_history_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(max_history_records.to_string()));

        Self {
            records,
            repository,
            settings,
            focus_handle,
            _focus_out_subscription,
            search_input,
            selected_index: 0,
            list_state,
            filtered_records: Vec::new(),
            copy_tx,
            show_settings: false,
            settings_activation_key_input,
            settings_max_history_input,
            selected_theme: theme_index,
            autostart_enabled,
        }
    }

    /// Copy content to clipboard
    fn copy_to_clipboard(&mut self, content: &str, content_type: &ContentType) {
        let request = match content_type {
            ContentType::Text => Some(crate::clipboard::CopyRequest::Text(content.to_string())),
            ContentType::Image => Some(crate::clipboard::CopyRequest::Image(content.to_string())),
            _ => None,
        };

        if let Some(req) = request {
            let _ = self.copy_tx.send(req);
        }
    }

    /// Clear clipboard history
    pub fn clear_history(&mut self) {
        if let Some(ref repo) = self.repository {
            if let Err(e) = repo.clear() {
                eprintln!("[ropy] Failed to clear clipboard history: {e}");
            } else {
                let mut guard = self.records.lock().unwrap();
                guard.clear();
            }
        }
    }

    /// Delete a single record by ID
    pub fn delete_record(&mut self, id: u64) {
        if let Some(ref repo) = self.repository {
            if let Err(e) = repo.delete(id) {
                eprintln!("[ropy] Failed to delete clipboard record: {e}");
            } else {
                let mut guard = self.records.lock().unwrap();
                guard.retain(|record| record.id != id);
            }
        }
    }

    /// Get filtered records based on search query
    fn get_filtered_records(&self, query: &str) -> Vec<ClipboardRecord> {
        if query.is_empty() {
            let guard = self.records.lock().unwrap();
            guard.clone()
        } else if let Some(ref repo) = self.repository {
            repo.search(query).unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    /// Confirm, hide and delete.
    fn confirm_record(&mut self, window: &mut Window, cx: &mut Context<Self>, index: usize) {
        let (id, content, content_type) = {
            if let Some(record) = self.filtered_records.get(index) {
                (
                    record.id,
                    record.content.clone(),
                    record.content_type.clone(),
                )
            } else {
                return;
            }
        };
        self.copy_to_clipboard(&content, &content_type);
        hide_window(window, cx);
        if matches!(content_type, ContentType::Image) || index != 0 {
            self.delete_record(id);
        }
    }

    fn save_settings(&mut self, cx: &mut Context<Self>, window: &mut Window) {
        let activation_key = self
            .settings_activation_key_input
            .read(cx)
            .value()
            .to_string();
        let max_history = self
            .settings_max_history_input
            .read(cx)
            .value()
            .to_string()
            .parse::<usize>()
            .unwrap_or(100);

        let theme = match self.selected_theme {
            0 => crate::config::AppTheme::Light,
            1 => crate::config::AppTheme::Dark,
            _ => crate::config::AppTheme::System,
        };

        {
            let mut settings = self.settings.write().unwrap();
            settings.hotkey.activation_key = activation_key.clone();
            settings.storage.max_history_records = max_history;
            settings.theme = theme.clone();
            settings.autostart.enabled = self.autostart_enabled;
            if let Err(e) = settings.save() {
                eprintln!("[ropy] Failed to save settings: {}", e);
            }
        }

        // Sync auto-start state with system
        if let Err(e) = self.sync_autostart_state() {
            eprintln!("[ropy] Failed to sync auto-start state: {}", e);
        }

        // Apply the new theme
        let app_theme = &theme.get_theme();
        crate::gui::app::set_app_theme(window, cx, app_theme);

        self.settings_max_history_input.update(cx, |input, cx| {
            input.set_placeholder(max_history.to_string(), window, cx);
            input.set_value("", window, cx);
        });
        self.settings_activation_key_input.update(cx, |input, cx| {
            input.set_placeholder(activation_key, window, cx);
            input.set_value("", window, cx);
        });
        cx.notify();
    }

    fn toggle_autostart(&mut self, cx: &mut Context<Self>) {
        self.autostart_enabled = !self.autostart_enabled;
        cx.notify();
    }

    fn sync_autostart_state(&self) -> Result<(), crate::config::AutoStartError> {
        let manager = crate::config::AutoStartManager::new("Ropy")?;
        manager.sync_state(self.autostart_enabled)?;
        Ok(())
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
            .p_4();

        if self.show_settings {
            return base.child(render_settings_content(self, cx));
        }

        // Render main clipboard view
        let query = self.search_input.read(cx).value().to_string();
        let new_filtered_records = self.get_filtered_records(&query);

        if new_filtered_records != self.filtered_records {
            self.filtered_records = new_filtered_records;
            self.list_state.reset(self.filtered_records.len());
        }

        if self.selected_index >= self.filtered_records.len() && !self.filtered_records.is_empty() {
            self.selected_index = self.filtered_records.len() - 1;
        } else if self.filtered_records.is_empty() {
            self.selected_index = 0;
        }

        base.on_action(cx.listener(Self::on_select_prev))
            .on_action(cx.listener(Self::on_select_next))
            .on_action(cx.listener(Self::on_confirm_selection))
            .on_key_down(cx.listener(Self::on_key_down))
            .child(render_header(cx))
            .child(render_search_input(&self.search_input, cx))
            .child(self.render_records_list(
                self.filtered_records.clone(),
                self.selected_index,
                self.list_state.clone(),
                cx,
            ))
    }
}

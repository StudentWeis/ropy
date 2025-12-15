mod render;
mod settings;

use crate::config::Settings;
use crate::gui::{active_window, hide_window};
use crate::repository::models::ContentType;
use crate::repository::{ClipboardRecord, ClipboardRepository};
use gpui::{
    AppContext, Context, Entity, FocusHandle, Focusable, ListAlignment, ListState, Render,
    Subscription, Window,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
};
use gpui_component::input::InputState;
use gpui_component::{ActiveTheme, v_flex};
use std::sync::{Arc, Mutex, RwLock};

// Re-export utilities for external use
use render::{render_header, render_records_list, render_search_input};
use settings::render_settings_content;

gpui::actions!(
    board,
    [Hide, Quit, Active, SelectPrev, SelectNext, ConfirmSelection]
);

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

    // Settings
    show_settings: bool,
    settings_activation_key_input: Entity<InputState>,
    settings_max_history_input: Entity<InputState>,
}

impl RopyBoard {
    pub fn new(
        records: Arc<Mutex<Vec<ClipboardRecord>>>,
        repository: Option<Arc<ClipboardRepository>>,
        settings: Arc<RwLock<Settings>>,
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

        let (max_history_records, activation_key) = {
            let settings_guard = settings.read().unwrap();
            (
                settings_guard.storage.max_history_records,
                settings_guard.hotkey.activation_key.clone(),
            )
        };
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
            show_settings: false,
            settings_activation_key_input,
            settings_max_history_input,
        }
    }

    /// Copy content to clipboard
    pub fn copy_to_clipboard(&mut self, content: &str, content_type: &ContentType) {
        let content = content.to_string();
        let content_type = content_type.clone();
        std::thread::spawn(move || match content_type {
            ContentType::Text => {
                let _ = crate::clipboard::copy_text(&content);
            }
            ContentType::Image => {
                let _ = crate::clipboard::copy_image(&content);
            }
            _ => {}
        });
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

    fn on_select_prev(&mut self, _: &SelectPrev, _window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.list_state.scroll_to_reveal_item(self.selected_index);
            cx.notify();
        }
    }

    fn on_select_next(&mut self, _: &SelectNext, _window: &mut Window, cx: &mut Context<Self>) {
        let query = self.search_input.read(cx).value().to_string();
        let count = self.get_filtered_records(&query).len();
        if count > 0 && self.selected_index < count - 1 {
            self.selected_index += 1;
            self.list_state.scroll_to_reveal_item(self.selected_index);
            cx.notify();
        }
    }

    fn on_confirm_selection(
        &mut self,
        _: &ConfirmSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.confirm_and_hide(window, cx);
    }

    fn confirm_and_hide(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.search_input.read(cx).value().to_string();
        let records = self.get_filtered_records(&query);
        if let Some(record) = records.get(self.selected_index) {
            self.copy_to_clipboard(&record.content, &record.content_type);
            hide_window(window, cx);
        }
    }

    fn on_active_action(&mut self, _: &Active, window: &mut Window, cx: &mut Context<Self>) {
        active_window(window, cx);
    }

    fn on_hide_action(&mut self, _: &Hide, window: &mut Window, cx: &mut Context<Self>) {
        // If the search input is focused, return focus to the main component before hiding
        if let Some(focused_handle) = window.focused(cx)
            && focused_handle == self.search_input.focus_handle(cx)
        {
            window.focus(&self.focus_handle);
            return;
        }
        hide_window(window, cx);
    }

    fn on_quit_action(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
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

        {
            let mut settings = self.settings.write().unwrap();
            settings.hotkey.activation_key = activation_key.clone();
            settings.storage.max_history_records = max_history;
            if let Err(e) = settings.save() {
                eprintln!("[ropy] Failed to save settings: {}", e);
            } else {
                println!("[ropy] Settings saved successfully");
            }
        }

        self.settings_max_history_input.update(cx, |input, cx| {
            input.set_placeholder(max_history.to_string(), window, cx);
            input.set_value("", window, cx);
        });
        self.settings_activation_key_input.update(cx, |input, cx| {
            input.set_placeholder(activation_key, window, cx);
            input.set_value("", window, cx);
        });
        self.show_settings = false;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    fn on_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // If the "/" key is pressed, focus the search input
        if event.keystroke.key.as_str() == "/" {
            window.focus(&self.search_input.focus_handle(cx));
            return;
        }
        // If the search input is focused, ignore key presses
        if let Some(focused_handle) = window.focused(cx)
            && focused_handle == self.search_input.focus_handle(cx)
        {
            return;
        }

        // Map number keys to record selection
        let key = &event.keystroke.key;
        let index = match key.as_str() {
            "1" => 0,
            "2" => 1,
            "3" => 2,
            "4" => 3,
            "5" => 4,
            _ => return,
        };
        self.selected_index = index;
        self.confirm_and_hide(window, cx);
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
            .child(render_records_list(
                self.filtered_records.clone(),
                self.selected_index,
                self.list_state.clone(),
                cx,
            ))
    }
}

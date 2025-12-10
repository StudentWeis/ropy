use crate::repository::{ClipboardRecord, ClipboardRepository};
use gpui::{Context, Entity, FocusHandle, Render, ScrollHandle, Subscription, Window, div, prelude::*};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, Sizable, h_flex, v_flex};
use std::sync::{Arc, Mutex};

gpui::actions!(board, [Hide, Quit, Active, SelectPrev, SelectNext, ConfirmSelection]);

/// RopyBoard Main Window Component
pub struct RopyBoard {
    records: Arc<Mutex<Vec<ClipboardRecord>>>,
    repository: Option<Arc<ClipboardRepository>>,
    focus_handle: FocusHandle,
    _focus_out_subscription: Subscription,
    search_input: Entity<InputState>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
}

impl RopyBoard {
    pub fn new(
        records: Arc<Mutex<Vec<ClipboardRecord>>>,
        repository: Option<Arc<ClipboardRepository>>,
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
        let scroll_handle = ScrollHandle::new();

        Self {
            records,
            repository,
            focus_handle,
            _focus_out_subscription,
            search_input,
            selected_index: 0,
            scroll_handle,
        }
    }

    /// Copy text to clipboard
    pub fn copy_to_clipboard(&mut self, text: &str) {
        crate::clipboard::copy_text(text).unwrap();
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
            self.scroll_handle.scroll_to_item(self.selected_index);
            cx.notify();
        }
    }

    fn on_select_next(&mut self, _: &SelectNext, _window: &mut Window, cx: &mut Context<Self>) {
        let query = self.search_input.read(cx).value().to_string();
        let count = self.get_filtered_records(&query).len();
        if count > 0 && self.selected_index < count - 1 {
            self.selected_index += 1;
            self.scroll_handle.scroll_to_item(self.selected_index);
            cx.notify();
        }
    }

    fn on_confirm_selection(
        &mut self,
        _: &ConfirmSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let query = self.search_input.read(cx).value().to_string();
        let records = self.get_filtered_records(&query);
        if let Some(record) = records.get(self.selected_index) {
            self.copy_to_clipboard(&record.content);
            hide_window(window, cx);
        }
    }

    fn on_active_action(&mut self, _: &Active, window: &mut Window, cx: &mut Context<Self>) {
        active_window(window, cx);
    }

    fn on_hide_action(&mut self, _: &Hide, window: &mut Window, cx: &mut Context<Self>) {
        hide_window(window, cx);
    }

    fn on_quit_action(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
        cx.quit();
    }
}

/// Create the "Clear" button element
fn create_clear_button(cx: &mut Context<'_, RopyBoard>) -> impl IntoElement {
    Button::new("clear-button")
        .small()
        .label("Clear All")
        .on_click(cx.listener(|this, _, _, _| {
            this.clear_history();
        }))
}

fn format_clipboard_content(record: &ClipboardRecord) -> String {
    if record.content.chars().count() > 100 {
        format!(
            "{}...",
            record.content.chars().take(100).collect::<String>()
        )
    } else {
        record.content.clone()
    }
}

/// Render the header section with title and clear button
fn render_header(cx: &mut Context<'_, RopyBoard>) -> impl IntoElement {
    h_flex()
        .justify_between()
        .items_center()
        .mb_4()
        .child(
            div()
                .text_lg()
                .text_color(cx.theme().foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child("Ropy"),
        )
        .child(create_clear_button(cx))
}

/// Render the search input section
fn render_search_input(search_input: &Entity<InputState>, cx: &mut Context<'_, RopyBoard>) -> impl IntoElement {
    v_flex().w_full().mb_4().child(
        Input::new(search_input)
            .appearance(false)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .px_3()
            .py_2()
    )
}

/// Render the scrollable list of clipboard records
fn render_records_list(
    records: Vec<ClipboardRecord>,
    selected_index: usize,
    scroll_handle: ScrollHandle,
    cx: &mut Context<'_, RopyBoard>,
) -> impl IntoElement {
    v_flex()
        .id("clipboard-list")
        .flex_1()
        .overflow_y_scroll()
        .track_scroll(&scroll_handle)
        .children(records.into_iter().enumerate().map(|(index, record)| {
            let display_content = format_clipboard_content(&record);
            let record_content = record.content.clone();
            let record_id = record.id;
            let is_selected = index == selected_index;
            
            v_flex()
                .w_full()
                .p_3()
                .mb_2()
                .bg(if is_selected { cx.theme().accent } else { cx.theme().secondary })
                .rounded_md()
                .border_1()
                .border_color(if is_selected { cx.theme().accent } else { cx.theme().border })
                .hover(|style| style.bg(cx.theme().accent).border_color(cx.theme().accent))
                .id(("record", index))
                .child(
                    h_flex()
                        .justify_between()
                        .items_start()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .cursor_pointer()
                                .id(("record-content", index))
                                .on_click(cx.listener(move |this: &mut RopyBoard, _event: &gpui::ClickEvent, window: &mut gpui::Window, cx: &mut gpui::Context<RopyBoard>| {
                                    this.copy_to_clipboard(&record_content);
                                    hide_window(window, cx);
                                }))
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().secondary_foreground)
                                        .line_height(gpui::relative(1.5))
                                        .child(display_content.clone()),
                                )
                                .child(
                                    div().text_xs().text_color(cx.theme().muted_foreground).mt_1().child(
                                        record.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                                    ),
                                )
                        )
                        .child(
                            Button::new(("delete-btn", index))
                                .xsmall()
                                .ghost()
                                .label("×")
                                .on_click(cx.listener(move |this: &mut RopyBoard, _event: &gpui::ClickEvent, _window: &mut gpui::Window, cx: &mut gpui::Context<RopyBoard>| {
                                    this.delete_record(record_id);
                                    cx.notify();
                                }))
                        )
                )
        }))
}

impl Render for RopyBoard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.search_input.read(cx).value().to_string();
        let records_clone = self.get_filtered_records(&query);
        
        if self.selected_index >= records_clone.len() && !records_clone.is_empty() {
            self.selected_index = records_clone.len() - 1;
        } else if records_clone.is_empty() {
            self.selected_index = 0;
        }

        v_flex()
            .id("ropy-board")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_hide_action))
            .on_action(cx.listener(Self::on_quit_action))
            .on_action(cx.listener(Self::on_active_action))
            .on_action(cx.listener(Self::on_select_prev))
            .on_action(cx.listener(Self::on_select_next))
            .on_action(cx.listener(Self::on_confirm_selection))
            .bg(cx.theme().background)
            .size_full()
            .p_4()
            .child(render_header(cx))
            .child(render_search_input(&self.search_input, cx))
            .child(render_records_list(records_clone, self.selected_index, self.scroll_handle.clone(), cx))
    }
}

pub fn hide_window<T>(_window: &mut Window, _cx: &mut gpui::Context<T>) {
    #[cfg(target_os = "windows")]
    _window.minimize_window();
    #[cfg(target_os = "macos")]
    _cx.hide();
}

pub fn active_window<T>(_window: &mut Window, _cx: &mut gpui::Context<T>) {
    #[cfg(target_os = "windows")]
    _window.activate_window();
    #[cfg(target_os = "macos")]
    _cx.activate(true);
}

use crate::repository::ClipboardRecord;
use crate::repository::ClipboardRepository;
use gpui::{Context, FocusHandle, Render, Subscription, Window, div, prelude::*, rgb};
use std::sync::{Arc, Mutex};

gpui::actions!(board, [Hide, Quit, Active]);

/// RopyBoard Main Window Component
pub struct RopyBoard {
    /// Clipboard history records
    pub records: Arc<Mutex<Vec<ClipboardRecord>>>,
    pub repository: Option<Arc<ClipboardRepository>>,
    pub focus_handle: FocusHandle,
    _focus_out_subscription: Subscription,
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
        Self {
            records,
            repository,
            focus_handle,
            _focus_out_subscription,
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

impl Render for RopyBoard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let records_guard = self.records.lock().unwrap();
        let records_clone: Vec<ClipboardRecord> = records_guard.clone();
        drop(records_guard);
        div()
            .id("ropy-board")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_hide_action))
            .on_action(cx.listener(Self::on_quit_action))
            .on_action(cx.listener(Self::on_active_action))
            .flex()
            .flex_col()
            .bg(rgb(0x2d2d2d))
            .size_full()
            .p_4()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .mb_4()
                    .child(
                        div()
                            .text_lg()
                            .text_color(rgb(0xffffff))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("Ropy"),
                    )
                    .child(
                        create_clear_button(cx),
                    ),
            )
            .child(
                div()
                    .id("clipboard-list")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .overflow_y_scroll()
                    .children(records_clone.into_iter().enumerate().map(|(index, record)| {
                        let display_content = format_clipboard_content(&record);
                        let record_content = record.content.clone();
                        let copy_callback = cx.listener(move |this: &mut RopyBoard, _event: &gpui::ClickEvent, window: &mut gpui::Window, cx: &mut gpui::Context<RopyBoard>| {
                            this.copy_to_clipboard(&record_content);
                            hide_window(window, cx);
                        });
                        div()
                            .flex()
                            .flex_col()
                            .w_full()
                            .p_3()
                            .mb_2()
                            .bg(rgb(0x3d3d3d))
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(0x4d4d4d))
                            .hover(|style| style.bg(rgb(0x4d4d4d)).border_color(rgb(0x6d6d6d)))
                            .cursor_pointer()
                            .id(("record", index))
                            .on_click(copy_callback)
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0xffffff))
                                    .overflow_hidden()
                                    .child(display_content),
                            )
                            .child(
                                div().text_xs().text_color(rgb(0x888888)).mt_1().child(
                                    record.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                                ),
                            )
                    })),
            )
    }
}

/// Create the "Clear" button element
fn create_clear_button(cx: &mut Context<'_, RopyBoard>) -> impl IntoElement {
    div()
        .px_3()
        .py_1()
        .bg(rgb(0x4d4d4d))
        .rounded_md()
        .text_sm()
        .text_color(rgb(0xffffff))
        .cursor_pointer()
        .hover(|style| style.bg(rgb(0x6d6d6d)))
        .id("clear-button")
        .on_click(cx.listener(|this, _, _, _| {
            this.clear_history();
        }))
        .child("清空")
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

pub fn hide_window(_window: &mut Window, _cx: &mut gpui::Context<RopyBoard>) {
    #[cfg(target_os = "windows")]
    _window.minimize_window();
    #[cfg(target_os = "macos")]
    _cx.hide();
}

pub fn active_window(_window: &mut Window, _cx: &mut gpui::Context<RopyBoard>) {
    #[cfg(target_os = "windows")]
    _window.activate_window();
    #[cfg(target_os = "macos")]
    _cx.activate(true);
}

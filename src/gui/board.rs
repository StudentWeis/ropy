use crate::repository::ClipboardRecord;
use gpui::{Context, Render, Window, div, prelude::*, rgb};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// RopyBoard 主应用结构体
pub struct RopyBoard {
    /// 剪切板历史记录列表
    pub records: Arc<Mutex<Vec<ClipboardRecord>>>,
    pub is_visible: Arc<AtomicBool>,
}

impl RopyBoard {
    pub fn new(records: Arc<Mutex<Vec<ClipboardRecord>>>, is_visible: Arc<AtomicBool>) -> Self {
        Self {
            records,
            is_visible,
        }
    }

    /// 复制文本到剪切板
    pub fn copy_to_clipboard(&mut self, text: &str) {
        // Use the decoupled API from the clipboard module
        match crate::clipboard::copy_text(text) {
            Ok(_) => {
                println!(
                    "[ropy] 已复制到剪切板: {}",
                    if text.len() > 50 {
                        format!("{}...", &text[..50])
                    } else {
                        text.to_string()
                    }
                );
            }
            Err(e) => {
                eprintln!("[ropy] 复制到剪切板失败: {}", e);
            }
        }
    }

    pub fn toggle_visibility(&self, cx: &mut gpui::Context<RopyBoard>) {
        let current_visible = self.is_visible.load(Ordering::Acquire);
        let new_visible = !current_visible;
        self.is_visible.store(new_visible, Ordering::Release);
        if new_visible {
            cx.activate(true);
        } else {
            cx.hide();
        }
    }
}

impl Render for RopyBoard {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let records_guard = self.records.lock().unwrap();
        let records_clone: Vec<ClipboardRecord> = records_guard.clone();
        drop(records_guard);

        div()
            .flex()
            .flex_col()
            .bg(rgb(0x2d2d2d))
            .size_full()
            .p_4()
            .child(
                div()
                    .text_lg()
                    .text_color(rgb(0xffffff))
                    .font_weight(gpui::FontWeight::BOLD)
                    .mb_4()
                    .child("RopyBoard"),
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
                        let copy_callback = cx.listener(move |this: &mut RopyBoard, _event: &gpui::ClickEvent, _window: &mut gpui::Window, cx: &mut gpui::Context<RopyBoard>| {
                            this.copy_to_clipboard(&record_content);
                            this.toggle_visibility(cx);
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

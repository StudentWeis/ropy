use crate::gui::utils::set_always_on_top;
#[cfg(target_os = "windows")]
use crate::gui::utils::start_window_drag;
use crate::repository::ClipboardRecord;
use crate::repository::models::ContentType;
use gpui::{
    Context, Entity, div, img, list,
    prelude::{InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled},
    px,
};
use gpui_component::Icon;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputState};
use gpui_component::{ActiveTheme, IconName, Sizable, h_flex, v_flex};
use regex::Regex;
use std::path::PathBuf;
use std::sync::OnceLock;

use super::RopyBoard;

fn get_hex_color(content: &str) -> Option<gpui::Rgba> {
    static HEX_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex =
        HEX_REGEX.get_or_init(|| Regex::new(r"^#([A-Fa-f0-9]{6}|[A-Fa-f0-9]{3})$").unwrap());

    if regex.is_match(content) {
        let hex = content.trim_start_matches('#');
        let value = if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            ((r as u32 * 17) << 16) | ((g as u32 * 17) << 8) | (b as u32 * 17)
        } else {
            u32::from_str_radix(hex, 16).ok()?
        };
        Some(gpui::rgb(value))
    } else {
        None
    }
}

/// Create the "Clear" button element
pub(super) fn create_clear_button(
    board: &RopyBoard,
    cx: &mut Context<'_, RopyBoard>,
) -> impl IntoElement {
    Button::new("clear-button")
        .ghost()
        .icon(Icon::empty().path("clear-all.svg"))
        .tooltip(board.i18n.t("clear_all"))
        .on_click(cx.listener(|this, _, _, _| {
            this.clear_history();
            this.clear_last_copy_state();
        }))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// Format clipboard content for display (truncate if too long)
pub(super) fn format_clipboard_content(record: &ClipboardRecord) -> String {
    if record.content.chars().count() > 100 {
        format!(
            "{}...",
            record.content.chars().take(100).collect::<String>()
        )
    } else {
        record.content.clone()
    }
}

/// Render the header section with title and settings/clear buttons
pub fn render_header(board: &RopyBoard, cx: &mut Context<'_, RopyBoard>) -> impl IntoElement {
    let is_pinned = board.pinned;
    let pin_tooltip = if is_pinned {
        board.i18n.t("unpin")
    } else {
        board.i18n.t("pin")
    };
    let header = h_flex().justify_between().items_center().mb_4().pt_4();

    #[cfg(target_os = "windows")]
    let header = header.on_mouse_down(gpui::MouseButton::Left, |_, window, cx| {
        start_window_drag(window, cx);
    });

    header
        .child(
            div()
                .text_lg()
                .text_color(cx.theme().foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child(board.i18n.t("app_name")),
        )
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    if is_pinned {
                        Button::new("pin-button").primary()
                    } else {
                        Button::new("pin-button").ghost()
                    }
                    .icon(Icon::empty().path("pin-to-top.svg"))
                    .tooltip(pin_tooltip)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.pinned = !this.pinned;
                        set_always_on_top(window, cx, this.pinned);
                        cx.notify();
                    }))
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|_, _, _, cx| cx.stop_propagation()),
                    ),
                )
                .child(
                    Button::new("settings-button")
                        .ghost()
                        .tooltip("Settings")
                        .icon(IconName::Settings)
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.show_settings = true;
                            window.focus(&this.focus_handle);
                            cx.notify();
                        }))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                )
                .child(create_clear_button(board, cx)),
        )
}

/// Render the search input section
pub(super) fn render_search_input(
    search_input: &Entity<InputState>,
    cx: &mut Context<'_, RopyBoard>,
) -> impl IntoElement {
    v_flex().w_full().mb_4().child(
        Input::new(search_input)
            .appearance(false)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .px_3()
            .py_2(),
    )
}

fn render_image_record(record: &ClipboardRecord) -> gpui::AnyElement {
    let path = PathBuf::from(record.content.clone());
    let file_stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let thumb_name = format!("{file_stem}_thumb.png");
    let thumb_path = path.parent().unwrap_or(&path).join(thumb_name);

    // Use thumbnail if exists, otherwise fallback to original
    let display_path = if thumb_path.exists() {
        thumb_path
    } else {
        path
    };
    img(display_path).max_h(px(100.0)).into_any_element()
}

fn render_text_record(cx: &mut gpui::App, record: &ClipboardRecord) -> gpui::AnyElement {
    let display_content = format_clipboard_content(record);
    let hex_color = get_hex_color(&record.content);

    let text_el = div()
        .text_sm()
        .text_color(cx.theme().secondary_foreground)
        .line_height(gpui::relative(1.5))
        .child(display_content);

    if let Some(color) = hex_color {
        h_flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .w_4()
                    .h_4()
                    .rounded_sm()
                    .bg(color)
                    .border_1()
                    .border_color(cx.theme().border),
            )
            .child(text_el)
            .into_any_element()
    } else {
        text_el.into_any_element()
    }
}

impl RopyBoard {
    /// Render the scrollable list of clipboard records
    pub fn render_records_list(&self, context: &mut Context<'_, RopyBoard>) -> impl IntoElement {
        let records = self.filtered_records.clone();
        let list_state = self.list_state.clone();
        let selected_index = self.selected_index;
        let view = context.weak_entity();
        list(list_state, move |index, _window, cx| {
            let record = &records[index];
            let record_id = record.id;
            let is_selected = index == selected_index;
            let content_type = record.content_type.clone();
            let view_click = view.clone();
            let view_delete = view.clone();

            div()
                .pb_2()
                .child(
                    v_flex()
                        .w_full()
                        .p_3()
                        .bg(if is_selected {
                            cx.theme().accent
                        } else {
                            cx.theme().secondary
                        })
                        .rounded_md()
                        .border_1()
                        .border_color(if is_selected {
                            cx.theme().accent
                        } else {
                            cx.theme().border
                        })
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
                                        .on_click(move |_event, window, cx| {
                                            view_click
                                                .update(cx, |this, cx| {
                                                    this.confirm_record(window, cx, index);
                                                })
                                                .ok();
                                        })
                                        .child(match content_type {
                                            ContentType::Text => render_text_record(cx, record),
                                            ContentType::Image => render_image_record(record),
                                            _ => div().child("Unknown content").into_any_element(),
                                        })
                                        .child(
                                            h_flex()
                                                .items_center()
                                                .gap_1()
                                                .mt_1()
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .bg(cx.theme().background)
                                                        .px_1()
                                                        .py_0()
                                                        .rounded_sm()
                                                        .child(format!("{}", index + 1)),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child(
                                                            record
                                                                .created_at
                                                                .format("%Y-%m-%d %H:%M:%S")
                                                                .to_string(),
                                                        ),
                                                ),
                                        ),
                                )
                                .child(
                                    Button::new(("delete-btn", index))
                                        .xsmall()
                                        .ghost()
                                        .label("×")
                                        .on_click(move |_event, _window, cx| {
                                            view_delete
                                                .update(cx, |this, cx| {
                                                    this.delete_record(record_id);
                                                    // TODO Delete associated last copy state
                                                    cx.notify();
                                                })
                                                .ok();
                                        }),
                                ),
                        ),
                )
                .into_any_element()
        })
        .w_full()
        .flex_1()
    }
}

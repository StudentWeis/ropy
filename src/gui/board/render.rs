use std::{path::PathBuf, sync::OnceLock};

use gpui::{
    Context, Entity, anchored, deferred, div, img, list,
    prelude::{
        FluentBuilder, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
        Styled,
    },
    px,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{Input, InputState},
    scroll::{Scrollbar, ScrollbarShow},
    v_flex,
};
use regex::Regex;

use super::{RopyBoard, preview};
use crate::{
    constants::APP_NAME,
    repository::{ClipboardRecord, models::ContentType},
};

fn get_hex_color(content: &str) -> Option<gpui::Rgba> {
    static HEX_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = HEX_REGEX.get_or_init(|| {
        Regex::new(r"^#([A-Fa-f0-9]{6}|[A-Fa-f0-9]{3})$").unwrap_or_else(|e| {
            tracing::error!(error = %e, "fatal: invalid hex color regex");
            // Fallback to a regex that matches nothing to avoid crash
            #[allow(clippy::unwrap_used)]
            Regex::new(r"a^").unwrap()
        })
    });

    if regex.is_match(content) {
        let hex = content.trim_start_matches('#');
        let value = if hex.len() == 3 {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            ((u32::from(r) * 17) << 16) | ((u32::from(g) * 17) << 8) | (u32::from(b) * 17)
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
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    Button::new("clear-button")
        .ghost()
        .icon(Icon::empty().path("icon/clear-all.svg"))
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
pub fn render_header(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let is_pinned = board.pinned;
    let pin_tooltip = if is_pinned {
        board.i18n.t("unpin")
    } else {
        board.i18n.t("pin")
    };
    let header = h_flex().justify_between().items_center().mb_4().pt_4();

    #[cfg(target_os = "windows")]
    let header = header.on_mouse_down(gpui::MouseButton::Left, |_, window, _cx| {
        crate::gui::utils::start_window_drag(window);
    });

    header
        .child(
            div()
                .text_lg()
                .text_color(cx.theme().foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child(APP_NAME),
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
                    .icon(Icon::empty().path("icon/pin-to-top.svg"))
                    .tooltip(pin_tooltip)
                    .on_click(
                        cx.listener(|this, _event, #[allow(unused_variables)] window, cx| {
                            this.pinned = !this.pinned;
                            #[cfg(not(target_os = "macos"))]
                            crate::gui::utils::set_always_on_top(window, this.pinned);
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|_, _, _, cx| cx.stop_propagation()),
                    ),
                )
                .child(
                    Button::new("help-button")
                        .ghost()
                        .icon(Icon::empty().path("icon/help.svg"))
                        .tooltip(board.i18n.t("help_title"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.show_help = true;
                            window.focus(&this.focus_handle);
                            cx.notify();
                        }))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                )
                .child(
                    Button::new("about-button")
                        .ghost()
                        .icon(Icon::empty().path("icon/info.svg"))
                        .tooltip(board.i18n.t("about_title"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.show_about = true;
                            window.focus(&this.focus_handle);
                            cx.notify();
                        }))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                )
                .child(
                    Button::new("settings-button")
                        .ghost()
                        .icon(Icon::empty().path("icon/settings.svg"))
                        .tooltip(board.i18n.t("settings_button"))
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
    cx: &Context<'_, RopyBoard>,
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

fn render_text_record(cx: &gpui::App, record: &ClipboardRecord) -> gpui::AnyElement {
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

fn create_preview(
    content_type: &ContentType,
    record_content: &str,
    window: &gpui::Window,
    cx: &mut gpui::App,
) -> gpui::AnyView {
    if content_type == &ContentType::Image {
        preview::image_tooltip(record_content, window, cx)
    } else {
        let content = if record_content.len() > 800 {
            record_content.chars().take(800).collect::<String>()
        } else {
            record_content.to_string()
        };
        preview::simple_tooltip(content, window, cx)
    }
}

#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
fn render_list_item(
    index: usize,
    record: &ClipboardRecord,
    is_selected: bool,
    show_preview: bool,
    hover_preview_enabled: bool,
    view: &gpui::WeakEntity<RopyBoard>,
    window: &gpui::Window,
    cx: &mut gpui::App,
) -> gpui::AnyElement {
    let record_id = record.id;
    let content_type = record.content_type.clone();
    let view_click = view.clone();
    let view_delete = view.clone();
    let view_pin = view.clone();
    let record_content = record.content.clone();
    let is_pinned = record.pinned;

    let preview_data = (content_type.clone(), record_content);

    let mut item = div().pb_2().relative().child(
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
                    .child({
                        let mut content_div = div()
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
                            });

                        if !show_preview && hover_preview_enabled {
                            content_div = content_div.tooltip({
                                let (content_type, record_content) = preview_data.clone();
                                move |window, cx| {
                                    create_preview(&content_type, &record_content, window, cx)
                                }
                            });
                        }

                        content_div
                            .child(match content_type {
                                ContentType::Text => render_text_record(cx, record),
                                ContentType::Image => render_image_record(record),
                                ContentType::FilePath => div().child("File").into_any_element(),
                            })
                            .child(
                                h_flex()
                                    .items_center()
                                    .gap_1()
                                    .mt_1()
                                    .when(is_pinned, |el: gpui::Div| {
                                        el.child(div().text_xs().child("📌"))
                                    })
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
                            )
                    })
                    .child(
                        h_flex().gap_1().child(
                            v_flex()
                                .gap_1()
                                .items_center()
                                .child({
                                    let btn = if is_pinned {
                                        Button::new(("pin-btn", index)).xsmall().primary()
                                    } else {
                                        Button::new(("pin-btn", index)).xsmall().ghost()
                                    };
                                    btn.icon(Icon::empty().path("icon/record-pin.svg"))
                                        .on_click(move |_event, _window, cx| {
                                            view_pin
                                                .update(cx, |this, cx| {
                                                    this.toggle_record_pin(record_id);
                                                    cx.notify();
                                                })
                                                .ok();
                                        })
                                })
                                .child(
                                    Button::new(("delete-btn", index))
                                        .xsmall()
                                        .ghost()
                                        .label("×")
                                        .on_click(move |_event, _window, cx| {
                                            view_delete
                                                .update(cx, |this, cx| {
                                                    this.delete_record(record_id);
                                                    cx.notify();
                                                })
                                                .ok();
                                        }),
                                ),
                        ),
                    ),
            ),
    );

    // Show preview if the item is selected and preview is enabled
    if hover_preview_enabled && is_selected && show_preview {
        let (content_type, record_content) = preview_data;
        item = item.child(
            deferred(
                div().absolute().top_full().left_0().child(
                    anchored()
                        .snap_to_window()
                        .child(div().mt_1().child(create_preview(
                            &content_type,
                            &record_content,
                            window,
                            cx,
                        ))),
                ),
            )
            .with_priority(1),
        );
    }

    item.into_any_element()
}

impl RopyBoard {
    /// Render the scrollable list of clipboard records
    pub fn render_records_list(&self, context: &Context<'_, Self>) -> impl IntoElement {
        let records = self.filtered_records.clone();
        let list_state = self.list_state.clone();
        let scrollbar_state = list_state.clone();
        let selected_index = self.selected_index;
        let show_preview = self.show_preview;
        let hover_preview_enabled = self.hover_preview_enabled;
        let view = context.weak_entity();

        div()
            .relative()
            .w_full()
            .flex_1()
            .child(
                list(list_state, move |index, window, cx| {
                    let record = &records[index];
                    let is_selected = index == selected_index;
                    render_list_item(
                        index,
                        record,
                        is_selected,
                        show_preview,
                        hover_preview_enabled,
                        &view,
                        window,
                        cx,
                    )
                })
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right(px(-10.0))
                    .bottom_0()
                    .child(
                        Scrollbar::vertical(&scrollbar_state)
                            .scrollbar_show(ScrollbarShow::Scrolling),
                    ),
            )
    }
}

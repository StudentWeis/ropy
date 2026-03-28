use std::{path::PathBuf, sync::OnceLock};

use gpui::{
    AnyElement, AnyView, App, Context, Window, anchored, deferred, div, img, list,
    prelude::{InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled},
    px,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    scroll::{Scrollbar, ScrollbarShow},
    v_flex,
};
use regex::Regex;

use super::{RopyBoard, preview};
use crate::repository::{ClipboardRecord, models::ContentType};

const LIST_CONTENT_PREVIEW_LIMIT: usize = 100;
const TOOLTIP_CONTENT_PREVIEW_LIMIT: usize = 800;

fn get_hex_color(content: &str) -> Option<gpui::Rgba> {
    static HEX_REGEX: OnceLock<Regex> = OnceLock::new();
    let regex = HEX_REGEX.get_or_init(|| {
        Regex::new(r"^#([A-Fa-f0-9]{6}|[A-Fa-f0-9]{3})$").unwrap_or_else(|e| {
            tracing::error!(error = %e, "fatal: invalid hex color regex");
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

fn truncate_content(content: &str, limit: usize) -> String {
    if content.chars().count() > limit {
        format!("{}...", content.chars().take(limit).collect::<String>())
    } else {
        content.to_string()
    }
}

fn render_image_record(record: &ClipboardRecord) -> AnyElement {
    let path = PathBuf::from(record.content.clone());
    let file_stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let thumb_name = format!("{file_stem}_thumb.png");
    let thumb_path = path.parent().unwrap_or(&path).join(thumb_name);

    let display_path = if thumb_path.exists() {
        thumb_path
    } else {
        path
    };
    img(display_path).max_h(px(100.0)).into_any_element()
}

fn render_text_record(cx: &App, record: &ClipboardRecord) -> AnyElement {
    let text = truncate_content(&record.content, LIST_CONTENT_PREVIEW_LIMIT);
    let text_element = div()
        .text_sm()
        .text_color(cx.theme().secondary_foreground)
        .line_height(gpui::relative(1.5))
        .child(text);

    if let Some(color) = get_hex_color(&record.content) {
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
            .child(text_element)
            .into_any_element()
    } else {
        text_element.into_any_element()
    }
}

fn create_preview(
    content_type: &ContentType,
    record_content: &str,
    window: &Window,
    cx: &mut App,
) -> AnyView {
    if content_type == &ContentType::Image {
        preview::image_tooltip(record_content, window, cx)
    } else {
        preview::simple_tooltip(
            truncate_content(record_content, TOOLTIP_CONTENT_PREVIEW_LIMIT),
            window,
            cx,
        )
    }
}

struct PreviewData {
    content_type: ContentType,
    record_content: String,
}

impl PreviewData {
    fn new(record: &ClipboardRecord) -> Self {
        Self {
            content_type: record.content_type.clone(),
            record_content: record.content.clone(),
        }
    }

    fn build(&self, window: &Window, cx: &mut App) -> AnyView {
        create_preview(&self.content_type, &self.record_content, window, cx)
    }
}

struct ItemStyle {
    selected_background: gpui::Hsla,
    normal_background: gpui::Hsla,
    border: gpui::Hsla,
    hover_border: gpui::Hsla,
}

impl ItemStyle {
    fn from_app(cx: &App) -> Self {
        Self {
            selected_background: cx.theme().accent,
            normal_background: cx.theme().secondary,
            border: cx.theme().border,
            hover_border: cx.theme().foreground,
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
struct RenderContext<'a> {
    index: usize,
    record: &'a ClipboardRecord,
    is_favorite: bool,
    is_selected: bool,
    show_preview: bool,
    hover_preview_enabled: bool,
    view: &'a gpui::WeakEntity<RopyBoard>,
}

fn render_record_body(
    ctx: &RenderContext<'_>,
    preview_data: &PreviewData,
    view_click: gpui::WeakEntity<RopyBoard>,
    cx: &App,
) -> AnyElement {
    let mut content = div()
        .flex_1()
        .min_w_0()
        .cursor_pointer()
        .id(("record-content", ctx.index))
        .on_click({
            let index = ctx.index;
            move |_event, window, cx| {
                view_click
                    .update(cx, |this, cx| {
                        this.confirm_record(window, cx, index);
                    })
                    .ok();
            }
        });

    if !ctx.show_preview && ctx.hover_preview_enabled {
        let preview_content_type = preview_data.content_type.clone();
        let preview_record_content = preview_data.record_content.clone();
        content = content.tooltip(move |window, cx| {
            create_preview(&preview_content_type, &preview_record_content, window, cx)
        });
    }

    content
        .child(match ctx.record.content_type {
            ContentType::Text => render_text_record(cx, ctx.record),
            ContentType::Image => render_image_record(ctx.record),
            ContentType::FilePath => div().child("File").into_any_element(),
        })
        .child(render_record_meta(ctx.index, ctx.record, cx))
        .into_any_element()
}

fn render_record_meta(index: usize, record: &ClipboardRecord, cx: &App) -> gpui::Div {
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
                .child((index + 1).to_string()),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(record.created_at.format("%Y-%m-%d %H:%M:%S").to_string()),
        )
}

fn render_record_actions(
    index: usize,
    record_id: u64,
    is_favorite: bool,
    is_pinned: bool,
    view_favorite: gpui::WeakEntity<RopyBoard>,
    view_pin: gpui::WeakEntity<RopyBoard>,
    view_delete: gpui::WeakEntity<RopyBoard>,
) -> gpui::Div {
    v_flex()
        .items_end()
        .gap(px(2.0))
        .child(
            h_flex()
                .gap(px(2.0))
                .items_center()
                .child({
                    let button = if is_favorite {
                        Button::new(("favorite-btn", index))
                            .xsmall()
                            .primary()
                            .label("★")
                    } else {
                        Button::new(("favorite-btn", index))
                            .xsmall()
                            .ghost()
                            .label("☆")
                    };
                    button.on_click(move |_event, _window, cx| {
                        view_favorite
                            .update(cx, |this, cx| {
                                this.toggle_record_favorite(record_id, cx);
                                cx.notify();
                            })
                            .ok();
                    })
                })
                .child({
                    let button = if is_pinned {
                        Button::new(("pin-btn", index)).xsmall().primary()
                    } else {
                        Button::new(("pin-btn", index)).xsmall().ghost()
                    };
                    button
                        .icon(Icon::empty().path("icon/record-pin.svg"))
                        .on_click(move |_event, _window, cx| {
                            view_pin
                                .update(cx, |this, cx| {
                                    this.toggle_record_pin(record_id, cx);
                                    cx.notify();
                                })
                                .ok();
                        })
                }),
        )
        .child(
            Button::new(("delete-btn", index))
                .xsmall()
                .ghost()
                .label("×")
                .on_click(move |_event, _window, cx| {
                    view_delete
                        .update(cx, |this, cx| {
                            this.delete_record(record_id, cx);
                            cx.notify();
                        })
                        .ok();
                }),
        )
}

fn render_selected_preview(
    preview_data: &PreviewData,
    window: &Window,
    cx: &mut App,
) -> impl IntoElement {
    deferred(
        div().absolute().top_full().left_0().child(
            anchored()
                .snap_to_window()
                .child(div().mt_1().child(preview_data.build(window, cx))),
        ),
    )
    .with_priority(1)
}

fn render_list_item(ctx: &RenderContext<'_>, window: &Window, cx: &mut App) -> AnyElement {
    let preview_data = PreviewData::new(ctx.record);
    let styles = ItemStyle::from_app(cx);
    let view_click = ctx.view.clone();
    let view_favorite = ctx.view.clone();
    let view_pin = ctx.view.clone();
    let view_delete = ctx.view.clone();
    let record_id = ctx.record.id;

    let mut item = div().pb_2().relative().child(
        v_flex()
            .w_full()
            .p_3()
            .bg(if ctx.is_selected {
                styles.selected_background
            } else {
                styles.normal_background
            })
            .rounded_md()
            .border_1()
            .border_color(if ctx.is_selected {
                styles.selected_background
            } else {
                styles.border
            })
            .hover(move |style| {
                let style = style.border_2().border_color(styles.hover_border);
                if ctx.is_selected {
                    style.bg(styles.selected_background)
                } else {
                    style
                }
            })
            .id(("record", ctx.index))
            .child(
                h_flex()
                    .justify_between()
                    .items_start()
                    .gap_2()
                    .child(render_record_body(ctx, &preview_data, view_click, cx))
                    .child(render_record_actions(
                        ctx.index,
                        record_id,
                        ctx.is_favorite,
                        ctx.record.pinned,
                        view_favorite,
                        view_pin,
                        view_delete,
                    )),
            ),
    );

    if ctx.hover_preview_enabled && ctx.is_selected && ctx.show_preview {
        item = item.child(render_selected_preview(&preview_data, window, cx));
    }

    item.into_any_element()
}

impl RopyBoard {
    #[allow(clippy::significant_drop_tightening)]
    pub fn render_records_list(&self, context: &Context<'_, Self>) -> impl IntoElement {
        let filtered_record_indices = self.filtered_record_indices.clone();
        let records = self.records.clone();
        let favorite_ids = self.favorite_ids.clone();
        let list_state = self.list_state.clone();
        let scrollbar_state = list_state.clone();
        let selected_index = self.selected_index;
        let show_preview = self.show_preview;
        let hover_preview_enabled = self.hover_preview_enabled && !self.show_clear_confirm;
        let view = context.weak_entity();

        div()
            .relative()
            .w_full()
            .flex_1()
            .child(
                list(list_state, move |index, window, cx| {
                    let Some(record_index) = filtered_record_indices.get(index).copied() else {
                        return div().into_any_element();
                    };
                    {
                        let guard = records
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        let Some(record) = guard.get(record_index) else {
                            return div().into_any_element();
                        };

                        render_list_item(
                            &RenderContext {
                                index,
                                record,
                                is_favorite: favorite_ids.contains(&record.id),
                                is_selected: index == selected_index,
                                show_preview,
                                hover_preview_enabled,
                                view: &view,
                            },
                            window,
                            cx,
                        )
                    }
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

#[cfg(test)]
mod tests {
    use super::truncate_content;

    #[test]
    fn truncate_content_preserves_short_text() {
        assert_eq!(truncate_content("abc", 5), "abc");
    }

    #[test]
    fn truncate_content_appends_ellipsis_for_long_text() {
        assert_eq!(truncate_content("abcdef", 3), "abc...");
    }
}

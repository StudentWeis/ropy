use std::path::{Path, PathBuf};

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

use super::{RopyBoard, preview};
use crate::{
    clipboard::thumb_path_for,
    gui::surface_with_opacity,
    repository::{ClipboardRecord, models::ContentType},
    utils::{deserialize_file_paths, read_or_recover},
};

const LIST_CONTENT_PREVIEW_LIMIT: usize = 80;
const TOOLTIP_CONTENT_PREVIEW_LIMIT: usize = 500;

fn get_hex_color(content: &str) -> Option<gpui::Rgba> {
    let hex = content.strip_prefix('#')?;
    let value = match hex.len() {
        3 if hex.chars().all(|ch| ch.is_ascii_hexdigit()) => {
            let r = u8::from_str_radix(&hex[0..1], 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2], 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3], 16).ok()?;
            ((u32::from(r) * 17) << 16) | ((u32::from(g) * 17) << 8) | (u32::from(b) * 17)
        }
        6 if hex.chars().all(|ch| ch.is_ascii_hexdigit()) => u32::from_str_radix(hex, 16).ok()?,
        _ => return None,
    };

    Some(gpui::rgb(value))
}

fn truncate_content(content: &str, limit: usize) -> String {
    if content.chars().count() > limit {
        format!("{}...", content.chars().take(limit).collect::<String>())
    } else {
        content.to_string()
    }
}

fn truncate_content_with_lines(content: &str, char_limit: usize, max_lines: usize) -> String {
    // First limit to max lines
    let lines: Vec<&str> = content.lines().take(max_lines).collect();
    let line_limited_content = lines.join("\n");

    // Then limit character count
    if line_limited_content.chars().count() > char_limit {
        format!(
            "{}...",
            line_limited_content
                .chars()
                .take(char_limit)
                .collect::<String>()
        )
    } else if content.lines().count() > max_lines {
        format!("{line_limited_content}...")
    } else {
        line_limited_content
    }
}

fn truncate_content_for_list(content: &str, limit: usize) -> String {
    truncate_content_with_lines(content, limit, 3)
}

fn truncate_content_for_preview(content: &str, limit: usize) -> String {
    truncate_content_with_lines(content, limit, 10)
}

fn render_image_record(record: &ClipboardRecord) -> AnyElement {
    let path = PathBuf::from(record.content.clone());
    let thumb_path = thumb_path_for(&path);

    let display_path = if thumb_path.exists() {
        thumb_path
    } else {
        path
    };
    img(display_path).max_h(px(100.0)).into_any_element()
}

fn render_text_record(cx: &App, record: &ClipboardRecord) -> AnyElement {
    // Remove leading blank lines and spaces
    let trimmed_content = record.content.trim_start();

    // Truncate content with line limit for list display
    let text = truncate_content_for_list(trimmed_content, LIST_CONTENT_PREVIEW_LIMIT);
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

fn file_display_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .filter(|name| !name.is_empty())
        .map_or_else(|| path.to_string(), ToString::to_string)
}

fn file_preview_content(record_content: &str) -> String {
    deserialize_file_paths(record_content).join("\n")
}

fn render_file_record(cx: &App, record: &ClipboardRecord) -> AnyElement {
    let files = deserialize_file_paths(&record.content);
    if files.is_empty() {
        return div()
            .text_sm()
            .text_color(cx.theme().secondary_foreground)
            .child("File")
            .into_any_element();
    }

    let title = if files.len() == 1 {
        file_display_name(&files[0])
    } else {
        format!("{} files", files.len())
    };
    let detail = if files.len() == 1 {
        files[0].clone()
    } else {
        files
            .iter()
            .take(2)
            .map(|path| file_display_name(path))
            .collect::<Vec<_>>()
            .join(", ")
    };

    v_flex()
        .gap_1()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().secondary_foreground)
                .child(title),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .line_height(gpui::relative(1.4))
                .child(truncate_content(&detail, LIST_CONTENT_PREVIEW_LIMIT)),
        )
        .into_any_element()
}

fn render_rich_text_badge(badge_background: gpui::Hsla, cx: &App) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap_1()
        .text_xs()
        .text_color(cx.theme().accent_foreground)
        .bg(badge_background)
        .px_1p5()
        .py_0p5()
        .rounded_sm()
        .child(Icon::empty().path("icon/filter-text.svg").size(px(12.0)))
}

fn render_file_badge(badge_background: gpui::Hsla, cx: &App) -> gpui::Div {
    div()
        .flex()
        .items_center()
        .gap_1()
        .text_xs()
        .text_color(cx.theme().accent_foreground)
        .bg(badge_background)
        .px_1p5()
        .py_0p5()
        .rounded_sm()
        .child(Icon::empty().path("icon/filter-files.svg").size(px(12.0)))
}

fn create_preview(
    content_type: &ContentType,
    record_content: &str,
    window: &Window,
    cx: &mut App,
) -> AnyView {
    match content_type {
        ContentType::Image => preview::image_tooltip(record_content, window, cx),
        ContentType::FilePath => preview::simple_tooltip(
            truncate_content_for_preview(
                &file_preview_content(record_content),
                TOOLTIP_CONTENT_PREVIEW_LIMIT,
            ),
            window,
            cx,
        ),
        ContentType::Text | ContentType::RichText => preview::simple_tooltip(
            truncate_content_for_preview(record_content, TOOLTIP_CONTENT_PREVIEW_LIMIT),
            window,
            cx,
        ),
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
    meta_background: gpui::Hsla,
    badge_background: gpui::Hsla,
}

impl ItemStyle {
    fn from_app(cx: &App, opacity_percent: u8) -> Self {
        Self {
            selected_background: surface_with_opacity(cx.theme().accent, opacity_percent),
            normal_background: surface_with_opacity(cx.theme().secondary, opacity_percent),
            border: surface_with_opacity(cx.theme().border, opacity_percent),
            hover_border: cx.theme().foreground,
            meta_background: surface_with_opacity(cx.theme().background, opacity_percent),
            badge_background: surface_with_opacity(cx.theme().accent, opacity_percent),
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
    opacity_percent: u8,
    view: &'a gpui::WeakEntity<RopyBoard>,
}

fn render_record_body(
    ctx: &RenderContext<'_>,
    preview_data: &PreviewData,
    view_click: gpui::WeakEntity<RopyBoard>,
    styles: &ItemStyle,
    cx: &App,
) -> AnyElement {
    let mut content = div()
        .flex_1()
        .min_w_0()
        .cursor_pointer()
        .id(("record-content", ctx.index))
        .on_click({
            let index = ctx.index;
            move |event, window, cx| {
                let confirm_as_plain_text = event.modifiers().shift;
                view_click
                    .update(cx, |this, cx| {
                        if confirm_as_plain_text {
                            this.confirm_record_as_plain_text(window, cx, index);
                        } else {
                            this.confirm_record(window, cx, index);
                        }
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
            ContentType::Text | ContentType::RichText => render_text_record(cx, ctx.record),
            ContentType::Image => render_image_record(ctx.record),
            ContentType::FilePath => render_file_record(cx, ctx.record),
        })
        .child(render_record_meta(
            ctx.index,
            ctx.record,
            styles.meta_background,
            styles.badge_background,
            cx,
        ))
        .into_any_element()
}

fn render_record_meta(
    index: usize,
    record: &ClipboardRecord,
    meta_background: gpui::Hsla,
    badge_background: gpui::Hsla,
    cx: &App,
) -> gpui::Div {
    let mut meta = h_flex()
        .items_center()
        .gap_1()
        .mt_1()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .bg(meta_background)
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
        );

    if record.content_type == ContentType::RichText {
        meta = meta.child(render_rich_text_badge(badge_background, cx));
    } else if record.content_type == ContentType::FilePath {
        meta = meta.child(render_file_badge(badge_background, cx));
    }

    meta
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
    let styles = ItemStyle::from_app(cx, ctx.opacity_percent);
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
                    .child(render_record_body(
                        ctx,
                        &preview_data,
                        view_click,
                        &styles,
                        cx,
                    ))
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
        let hover_preview_enabled =
            self.settings_editor.hover_preview_enabled && !self.show_clear_confirm;
        let opacity_percent = self.settings_editor.window_opacity_percent;
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
                        let guard = read_or_recover(&records);
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
                                opacity_percent,
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
    use super::{file_display_name, file_preview_content, get_hex_color, truncate_content};

    #[test]
    fn truncate_content_preserves_short_text() {
        assert_eq!(truncate_content("abc", 5), "abc");
    }

    #[test]
    fn truncate_content_appends_ellipsis_for_long_text() {
        assert_eq!(truncate_content("abcdef", 3), "abc...");
    }

    #[test]
    fn get_hex_color_accepts_short_and_long_hex_values() {
        assert_eq!(get_hex_color("#abc"), Some(gpui::rgb(0xAA_BB_CC)));
        assert_eq!(get_hex_color("#A1b2C3"), Some(gpui::rgb(0xA1_B2_C3)));
    }

    #[test]
    fn get_hex_color_rejects_invalid_hex_values() {
        assert_eq!(get_hex_color("abc"), None);
        assert_eq!(get_hex_color("#abcd"), None);
        assert_eq!(get_hex_color("#12x456"), None);
    }

    #[test]
    fn test_file_display_name_when_path_has_filename_returns_filename() {
        assert_eq!(file_display_name("/tmp/archive.zip"), "archive.zip");
    }

    #[test]
    fn test_file_preview_content_when_json_array_returns_joined_paths() {
        let preview = file_preview_content("[\"/tmp/a.txt\",\"/tmp/b.txt\"]");

        assert_eq!(preview, "/tmp/a.txt\n/tmp/b.txt");
    }

    #[test]
    fn test_file_preview_content_when_legacy_string_returns_single_path() {
        let preview = file_preview_content("/tmp/a.txt");

        assert_eq!(preview, "/tmp/a.txt");
    }
}

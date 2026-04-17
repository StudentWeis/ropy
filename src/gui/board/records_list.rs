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
    config::LayoutMode,
    gui::surface_with_opacity,
    repository::{ClipboardRecord, models::ContentType},
    utils::{deserialize_file_paths, read_or_recover},
};

const GRID_COLUMN_COUNT: usize = 2;
const GRID_CONTENT_PREVIEW_LIMIT: usize = 44;
const GRID_CARD_HEIGHT: f32 = 120.0;
const LIST_CONTENT_PREVIEW_LIMIT: usize = 80;
const TOOLTIP_CONTENT_PREVIEW_LIMIT: usize = 500;

pub(super) const fn visible_list_len(record_count: usize, layout_mode: LayoutMode) -> usize {
    match layout_mode {
        LayoutMode::List => record_count,
        LayoutMode::Grid => {
            if record_count == 0 {
                0
            } else {
                record_count.div_ceil(GRID_COLUMN_COUNT)
            }
        }
    }
}

pub(super) const fn list_row_for_selected_index(
    selected_index: usize,
    layout_mode: LayoutMode,
) -> usize {
    match layout_mode {
        LayoutMode::List => selected_index,
        LayoutMode::Grid => selected_index / GRID_COLUMN_COUNT,
    }
}

const fn row_start_index(row_index: usize, layout_mode: LayoutMode) -> usize {
    match layout_mode {
        LayoutMode::List => row_index,
        LayoutMode::Grid => row_index * GRID_COLUMN_COUNT,
    }
}

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

fn truncate_content_for_grid(content: &str, limit: usize) -> String {
    truncate_content_with_lines(content, limit, 2)
}

fn truncate_content_for_preview(content: &str, limit: usize) -> String {
    truncate_content_with_lines(content, limit, 10)
}

fn render_image_record(record: &ClipboardRecord, compact: bool) -> AnyElement {
    let path = PathBuf::from(record.content.clone());
    let thumb_path = thumb_path_for(&path);

    let display_path = if thumb_path.exists() {
        thumb_path
    } else {
        path
    };
    let max_height = if compact { 72.0 } else { 100.0 };

    img(display_path).max_h(px(max_height)).into_any_element()
}

fn render_text_record(cx: &App, record: &ClipboardRecord, compact: bool) -> AnyElement {
    // Remove leading blank lines and spaces
    let trimmed_content = record.content.trim_start();

    // Truncate content with line limit for list display
    let text = if compact {
        truncate_content_for_grid(trimmed_content, GRID_CONTENT_PREVIEW_LIMIT)
    } else {
        truncate_content_for_list(trimmed_content, LIST_CONTENT_PREVIEW_LIMIT)
    };
    let text_element = div()
        .text_sm()
        .text_color(cx.theme().secondary_foreground)
        .line_height(gpui::relative(if compact { 1.35 } else { 1.5 }))
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

fn render_file_record(cx: &App, record: &ClipboardRecord, compact: bool) -> AnyElement {
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
            .take(if compact { 1 } else { 2 })
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
                .child(truncate_content(
                    &detail,
                    if compact {
                        GRID_CONTENT_PREVIEW_LIMIT
                    } else {
                        LIST_CONTENT_PREVIEW_LIMIT
                    },
                )),
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
    layout_mode: LayoutMode,
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
    let compact = ctx.layout_mode == LayoutMode::Grid;
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

    content = content.child(match ctx.record.content_type {
        ContentType::Text | ContentType::RichText => render_text_record(cx, ctx.record, compact),
        ContentType::Image => render_image_record(ctx.record, compact),
        ContentType::FilePath => render_file_record(cx, ctx.record, compact),
    });

    if !compact {
        content = content.child(render_record_meta(
            ctx.index,
            ctx.record,
            styles.meta_background,
            styles.badge_background,
            true,
            true,
            cx,
        ));
    }

    content.into_any_element()
}

fn render_record_meta(
    index: usize,
    record: &ClipboardRecord,
    meta_background: gpui::Hsla,
    badge_background: gpui::Hsla,
    show_timestamp: bool,
    with_top_margin: bool,
    cx: &App,
) -> gpui::Div {
    let mut meta = h_flex().items_center().gap_1();

    if with_top_margin {
        meta = meta.mt_1();
    }

    meta = meta.child(
        div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .bg(meta_background)
            .px_1()
            .py_0()
            .rounded_sm()
            .child((index + 1).to_string()),
    );

    if show_timestamp {
        meta = meta.child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(record.created_at.format("%Y-%m-%d %H:%M:%S").to_string()),
        );
    }

    if record.content_type == ContentType::RichText {
        meta = meta.child(render_rich_text_badge(badge_background, cx));
    } else if record.content_type == ContentType::FilePath {
        meta = meta.child(render_file_badge(badge_background, cx));
    }

    meta
}

fn render_grid_record_header(
    ctx: &RenderContext<'_>,
    styles: &ItemStyle,
    record_id: u64,
    view_favorite: gpui::WeakEntity<RopyBoard>,
    view_pin: gpui::WeakEntity<RopyBoard>,
    view_delete: gpui::WeakEntity<RopyBoard>,
    cx: &App,
) -> gpui::Div {
    h_flex()
        .w_full()
        .justify_between()
        .items_start()
        .gap_2()
        .child(
            div().flex_1().min_w_0().child(render_record_meta(
                ctx.index,
                ctx.record,
                styles.meta_background,
                styles.badge_background,
                false,
                false,
                cx,
            )),
        )
        .child(render_record_actions(
            ctx.index,
            record_id,
            ctx.is_favorite,
            ctx.record.pinned,
            view_favorite,
            view_pin,
            view_delete,
            true,
        ))
}

fn render_record_actions(
    index: usize,
    record_id: u64,
    is_favorite: bool,
    is_pinned: bool,
    view_favorite: gpui::WeakEntity<RopyBoard>,
    view_pin: gpui::WeakEntity<RopyBoard>,
    view_delete: gpui::WeakEntity<RopyBoard>,
    compact: bool,
) -> AnyElement {
    let favorite_button = {
        let button = if is_favorite {
            Button::new(("favorite-btn", index)).xsmall().primary().label("★")
        } else {
            Button::new(("favorite-btn", index)).xsmall().ghost().label("☆")
        };
        button.on_click(move |_event, _window, cx| {
            view_favorite
                .update(cx, |this, cx| {
                    this.toggle_record_favorite(record_id, cx);
                    cx.notify();
                })
                .ok();
        })
    };

    let pin_button = {
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
    };

    let delete_button = Button::new(("delete-btn", index))
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
        });

    if compact {
        h_flex()
            .gap(px(2.0))
            .items_center()
            .child(favorite_button)
            .child(pin_button)
            .child(delete_button)
            .into_any_element()
    } else {
        v_flex()
            .items_end()
            .gap(px(2.0))
            .child(
                h_flex()
                    .gap(px(2.0))
                    .items_center()
                    .child(favorite_button)
                    .child(pin_button),
            )
            .child(delete_button)
            .into_any_element()
    }
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
    let compact = ctx.layout_mode == LayoutMode::Grid;
    let preview_data = PreviewData::new(ctx.record);
    let styles = ItemStyle::from_app(cx, ctx.opacity_percent);
    let view_click = ctx.view.clone();
    let view_favorite = ctx.view.clone();
    let view_pin = ctx.view.clone();
    let view_delete = ctx.view.clone();
    let record_id = ctx.record.id;

    let card = if compact {
        v_flex()
            .w_full()
            .h(px(GRID_CARD_HEIGHT))
            .px_2()
            .py_1()
            .bg(styles.normal_background)
            .rounded_md()
            .border_color(if ctx.is_selected {
                styles.hover_border
            } else {
                styles.border
            })
            .border_1()
            .hover(move |style| {
                if ctx.is_selected {
                    style
                } else {
                    style
                        .bg(styles.selected_background)
                        .border_color(styles.selected_background)
                }
            })
            .id(("record", ctx.index))
            .child(
                v_flex()
                    .w_full()
                    .gap_1()
                    .child(render_grid_record_header(
                        ctx,
                        &styles,
                        record_id,
                        view_favorite,
                        view_pin,
                        view_delete,
                        cx,
                    ))
                    .child(render_record_body(
                        ctx,
                        &preview_data,
                        view_click,
                        &styles,
                        cx,
                    )),
            )
    } else {
        v_flex()
            .w_full()
            .p_3()
            .bg(styles.normal_background)
            .rounded_md()
            .border_color(if ctx.is_selected {
                styles.hover_border
            } else {
                styles.border
            })
            .border_1()
            .hover(move |style| {
                if ctx.is_selected {
                    style
                } else {
                    style
                        .bg(styles.selected_background)
                        .border_color(styles.selected_background)
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
                        false,
                    )),
            )
    };

    let mut item = if compact {
        div().relative().min_w_0().child(card)
    } else {
        div().pb_2().relative().child(card)
    };

    if ctx.hover_preview_enabled && ctx.is_selected && ctx.show_preview {
        item = item.child(render_selected_preview(&preview_data, window, cx));
    }

    item.into_any_element()
}

fn render_grid_row(
    row_index: usize,
    filtered_record_indices: &[usize],
    records: &crate::repository::SharedRecords,
    favorite_ids: &std::collections::HashSet<u64>,
    selected_index: usize,
    show_preview: bool,
    hover_preview_enabled: bool,
    opacity_percent: u8,
    view: &gpui::WeakEntity<RopyBoard>,
    window: &Window,
    cx: &mut App,
) -> AnyElement {
    let first_index = row_start_index(row_index, LayoutMode::Grid);
    let second_index = first_index + 1;
    let guard = read_or_recover(records);

    let Some(first_record_index) = filtered_record_indices.get(first_index).copied() else {
        return div().into_any_element();
    };
    let Some(first_record) = guard.get(first_record_index) else {
        return div().into_any_element();
    };

    let mut row = div()
        .flex()
        .flex_row()
        .w_full()
        .gap_2()
        .pb_2()
        .child(
            div().flex_1().min_w_0().child(render_list_item(
                &RenderContext {
                    index: first_index,
                    record: first_record,
                    is_favorite: favorite_ids.contains(&first_record.id),
                    is_selected: first_index == selected_index,
                    layout_mode: LayoutMode::Grid,
                    show_preview,
                    hover_preview_enabled,
                    opacity_percent,
                    view,
                },
                window,
                cx,
            )),
        );

    if let Some(second_record_index) = filtered_record_indices.get(second_index).copied()
        && let Some(second_record) = guard.get(second_record_index)
    {
        row = row.child(
            div().flex_1().min_w_0().child(render_list_item(
                &RenderContext {
                    index: second_index,
                    record: second_record,
                    is_favorite: favorite_ids.contains(&second_record.id),
                    is_selected: second_index == selected_index,
                    layout_mode: LayoutMode::Grid,
                    show_preview,
                    hover_preview_enabled,
                    opacity_percent,
                    view,
                },
                window,
                cx,
            )),
        );
    } else {
        row = row.child(div().flex_1());
    }

    row.into_any_element()
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
        let layout_mode = self.layout_mode;
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
                    if layout_mode == LayoutMode::Grid {
                        return render_grid_row(
                            index,
                            filtered_record_indices.as_ref(),
                            &records,
                            favorite_ids.as_ref(),
                            selected_index,
                            show_preview,
                            hover_preview_enabled,
                            opacity_percent,
                            &view,
                            window,
                            cx,
                        );
                    }

                    let Some(record_index) = filtered_record_indices.get(index).copied() else {
                        return div().into_any_element();
                    };
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
                            layout_mode: LayoutMode::List,
                            show_preview,
                            hover_preview_enabled,
                            opacity_percent,
                            view: &view,
                        },
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

#[cfg(test)]
mod tests {
    use super::{
        file_display_name, file_preview_content, get_hex_color, list_row_for_selected_index,
        row_start_index, truncate_content, visible_list_len,
    };
    use crate::config::LayoutMode;

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

    #[test]
    fn test_visible_list_len_uses_record_count_for_list_mode() {
        assert_eq!(visible_list_len(0, LayoutMode::List), 0);
        assert_eq!(visible_list_len(3, LayoutMode::List), 3);
    }

    #[test]
    fn test_visible_list_len_rounds_up_rows_for_grid_mode() {
        assert_eq!(visible_list_len(0, LayoutMode::Grid), 0);
        assert_eq!(visible_list_len(1, LayoutMode::Grid), 1);
        assert_eq!(visible_list_len(2, LayoutMode::Grid), 1);
        assert_eq!(visible_list_len(3, LayoutMode::Grid), 2);
        assert_eq!(visible_list_len(5, LayoutMode::Grid), 3);
    }

    #[test]
    fn test_list_row_for_selected_index_maps_grid_selection_to_row() {
        assert_eq!(list_row_for_selected_index(0, LayoutMode::List), 0);
        assert_eq!(list_row_for_selected_index(3, LayoutMode::List), 3);
        assert_eq!(list_row_for_selected_index(0, LayoutMode::Grid), 0);
        assert_eq!(list_row_for_selected_index(1, LayoutMode::Grid), 0);
        assert_eq!(list_row_for_selected_index(2, LayoutMode::Grid), 1);
        assert_eq!(list_row_for_selected_index(5, LayoutMode::Grid), 2);
    }

    #[test]
    fn test_row_start_index_maps_rows_to_first_record_in_each_layout() {
        assert_eq!(row_start_index(0, LayoutMode::List), 0);
        assert_eq!(row_start_index(3, LayoutMode::List), 3);
        assert_eq!(row_start_index(0, LayoutMode::Grid), 0);
        assert_eq!(row_start_index(1, LayoutMode::Grid), 2);
        assert_eq!(row_start_index(3, LayoutMode::Grid), 6);
    }
}

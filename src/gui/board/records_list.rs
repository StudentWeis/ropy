use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};

use gpui::{
    AnyElement, AnyView, App, Context, RenderOnce, ScrollHandle, Window, anchored, deferred, div,
    img, list,
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

use super::{
    RopyBoard,
    color::{ClipboardColor, parse_clipboard_color},
    preview,
};
use crate::{
    clipboard::thumb_path_for,
    config::LayoutMode,
    gui::surface_with_opacity,
    repository::{ClipboardRecord, models::ContentType},
    utils::{deserialize_file_paths, read_or_recover},
};

const GRID_COLUMN_COUNT: usize = 2;
const GRID_COLUMN_GAP: f32 = 8.0;
const GRID_ROW_GAP: f32 = 8.0;
const GRID_CONTENT_PREVIEW_LIMIT: usize = 120;
const GRID_CONTENT_PREVIEW_MAX_LINES: usize = 5;
const GRID_CARD_MIN_HEIGHT: f32 = 112.0;
const GRID_CARD_MAX_HEIGHT: f32 = 168.0;
const GRID_IMAGE_MAX_HEIGHT: f32 = 96.0;
const GRID_COLOR_SWATCH_HEIGHT: f32 = 60.0;
const GRID_COLOR_SWATCH_GAP: f32 = 8.0;
const GRID_OVERSCAN_PX: f32 = 240.0;
const GRID_ESTIMATED_CARD_CHROME_HEIGHT: f32 = 48.0;
const GRID_ESTIMATED_TEXT_LINE_HEIGHT: f32 = 18.0;
const GRID_ESTIMATED_FILE_TITLE_HEIGHT: f32 = 18.0;
const GRID_ESTIMATED_FILE_DETAIL_LINE_HEIGHT: f32 = 16.0;
const GRID_ESTIMATED_TEXT_LINE_WIDTH_UNITS: f32 = 16.0;
const BOARD_HORIZONTAL_PADDING: f32 = 32.0;
const SCROLLBAR_OVERLAY_RIGHT_OFFSET: f32 = -10.0;
const LIST_CONTENT_PREVIEW_LIMIT: usize = 80;
const LIST_COLOR_SWATCH_SIZE_PX: f32 = 16.0;
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
    truncate_content_with_lines(content, limit, GRID_CONTENT_PREVIEW_MAX_LINES)
}

fn truncate_content_for_preview(content: &str, limit: usize) -> String {
    truncate_content_with_lines(content, limit, 10)
}

fn estimated_text_units(content: &str) -> f32 {
    content
        .chars()
        .map(|ch| {
            if ch.is_ascii_whitespace() {
                0.35
            } else if ch.is_ascii_punctuation() {
                0.5
            } else if ch.is_ascii() {
                0.65
            } else {
                1.1
            }
        })
        .sum()
}

fn estimated_wrapped_line_count(content: &str, max_lines: usize) -> usize {
    let line_count = content
        .lines()
        .take(max_lines)
        .map(|line| {
            if line.is_empty() {
                1
            } else {
                (estimated_text_units(line) / GRID_ESTIMATED_TEXT_LINE_WIDTH_UNITS)
                    .ceil()
                    .max(1.0) as usize
            }
        })
        .sum::<usize>();

    line_count.clamp(1, max_lines)
}

fn estimated_grid_text_lines(content: &str) -> usize {
    estimated_wrapped_line_count(
        &truncate_content_for_grid(content.trim_start(), GRID_CONTENT_PREVIEW_LIMIT),
        GRID_CONTENT_PREVIEW_MAX_LINES,
    )
}

fn estimated_grid_color_body_height(content: &str) -> f32 {
    GRID_ESTIMATED_TEXT_LINE_HEIGHT.mul_add(
        estimated_grid_text_lines(content) as f32,
        GRID_COLOR_SWATCH_HEIGHT + GRID_COLOR_SWATCH_GAP,
    )
}

fn estimated_grid_file_detail_lines(record_content: &str) -> usize {
    let files = deserialize_file_paths(record_content);
    let detail = if files.len() <= 1 {
        files.first().cloned().unwrap_or_default()
    } else {
        files
            .iter()
            .take(1)
            .map(|path| file_display_name(path))
            .collect::<Vec<_>>()
            .join(", ")
    };

    estimated_wrapped_line_count(
        &truncate_content(&detail, GRID_CONTENT_PREVIEW_LIMIT),
        GRID_CONTENT_PREVIEW_MAX_LINES.saturating_sub(1),
    )
}

fn estimated_grid_card_height(record: &ClipboardRecord) -> f32 {
    let body_height = match record.content_type {
        ContentType::Text | ContentType::RichText => {
            if parse_clipboard_color(&record.content).is_some() {
                estimated_grid_color_body_height(&record.content)
            } else {
                estimated_grid_text_lines(&record.content) as f32 * GRID_ESTIMATED_TEXT_LINE_HEIGHT
            }
        }
        ContentType::Image => GRID_IMAGE_MAX_HEIGHT,
        ContentType::FilePath => GRID_ESTIMATED_FILE_DETAIL_LINE_HEIGHT.mul_add(
            estimated_grid_file_detail_lines(&record.content) as f32,
            GRID_ESTIMATED_FILE_TITLE_HEIGHT,
        ),
    };

    (GRID_ESTIMATED_CARD_CHROME_HEIGHT + body_height)
        .clamp(GRID_CARD_MIN_HEIGHT, GRID_CARD_MAX_HEIGHT)
}

fn render_image_record(record: &ClipboardRecord, compact: bool) -> AnyElement {
    let path = PathBuf::from(record.content.clone());
    let thumb_path = thumb_path_for(&path);

    let display_path = if thumb_path.exists() {
        thumb_path
    } else {
        path
    };
    let max_height = if compact {
        GRID_IMAGE_MAX_HEIGHT
    } else {
        100.0
    };

    img(display_path).max_h(px(max_height)).into_any_element()
}

fn render_color_swatch(color: ClipboardColor, compact: bool, cx: &App) -> gpui::Div {
    let swatch = div()
        .bg(color.to_gpui_rgba())
        .border_1()
        .border_color(cx.theme().border);

    if compact {
        swatch.w_full().h(px(GRID_COLOR_SWATCH_HEIGHT)).rounded_md()
    } else {
        swatch
            .w(px(LIST_COLOR_SWATCH_SIZE_PX))
            .h(px(LIST_COLOR_SWATCH_SIZE_PX))
            .rounded_sm()
    }
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
        .w_full()
        .min_w_0()
        .text_sm()
        .text_color(cx.theme().secondary_foreground)
        .line_height(gpui::relative(if compact { 1.35 } else { 1.5 }))
        .child(text);

    if let Some(color) = parse_clipboard_color(&record.content) {
        if compact {
            v_flex()
                .w_full()
                .gap(px(GRID_COLOR_SWATCH_GAP))
                .child(render_color_swatch(color, true, cx))
                .child(text_element)
                .into_any_element()
        } else {
            h_flex()
                .items_center()
                .gap_2()
                .child(render_color_swatch(color, false, cx))
                .child(text_element)
                .into_any_element()
        }
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

struct RecordsListState {
    filtered_record_indices: Arc<Vec<usize>>,
    records: crate::repository::SharedRecords,
    favorite_ids: Arc<HashSet<u64>>,
    selected_index: usize,
    layout_mode: LayoutMode,
    show_preview: bool,
    hover_preview_enabled: bool,
    opacity_percent: u8,
    view: gpui::WeakEntity<RopyBoard>,
}

impl RecordsListState {
    fn from_board(board: &RopyBoard, context: &Context<'_, RopyBoard>) -> Self {
        Self {
            filtered_record_indices: board.filtered_record_indices.clone(),
            records: board.records.clone(),
            favorite_ids: board.favorite_ids.clone(),
            selected_index: board.selected_index,
            layout_mode: board.layout_mode,
            show_preview: board.show_preview,
            hover_preview_enabled: board.settings_editor.hover_preview_enabled
                && !board.show_clear_confirm,
            opacity_percent: board.settings_editor.window_opacity_percent,
            view: context.weak_entity(),
        }
    }

    fn render_row(&self, index: usize, window: &Window, cx: &mut App) -> AnyElement {
        self.render_list_row(index, window, cx)
    }

    fn render_list_row(&self, index: usize, window: &Window, cx: &mut App) -> AnyElement {
        let Some(record) = self.record_for_filtered_index(index) else {
            return div().into_any_element();
        };

        render_list_item(&self.render_context(index, &record), window, cx)
    }

    fn record_for_filtered_index(&self, filtered_index: usize) -> Option<ClipboardRecord> {
        let record_index = self.filtered_record_indices.get(filtered_index).copied()?;
        let guard = read_or_recover(&self.records);
        guard.get(record_index).cloned()
    }

    fn render_context<'a>(
        &'a self,
        index: usize,
        record: &'a ClipboardRecord,
    ) -> RenderContext<'a> {
        RenderContext {
            index,
            record,
            is_favorite: self.favorite_ids.contains(&record.id),
            is_selected: index == self.selected_index,
            layout_mode: self.layout_mode,
            show_preview: self.show_preview,
            hover_preview_enabled: self.hover_preview_enabled,
            opacity_percent: self.opacity_percent,
            view: &self.view,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct MasonryPlacement {
    index: usize,
    column: usize,
    left: f32,
    top: f32,
    height: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct MasonryLayout {
    placements: Vec<MasonryPlacement>,
    total_height: f32,
}

fn build_masonry_layout(
    item_heights: &[f32],
    column_count: usize,
    column_width: f32,
    column_gap: f32,
    row_gap: f32,
) -> MasonryLayout {
    let mut column_heights = vec![0.0; column_count];
    let mut placements = Vec::with_capacity(item_heights.len());

    for (index, height) in item_heights.iter().copied().enumerate() {
        let column = column_heights
            .iter()
            .enumerate()
            .min_by(|(_, left_height), (_, right_height)| {
                left_height
                    .partial_cmp(right_height)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map_or(0, |(column, _)| column);

        let top = column_heights[column];
        let left = column as f32 * (column_width + column_gap);
        column_heights[column] += height + row_gap;

        placements.push(MasonryPlacement {
            index,
            column,
            left,
            top,
            height,
        });
    }

    let total_height = if placements.is_empty() {
        0.0
    } else {
        (column_heights.into_iter().fold(0.0, f32::max) - row_gap).max(0.0)
    };

    MasonryLayout {
        placements,
        total_height,
    }
}

fn grid_available_width(window: &Window) -> gpui::Pixels {
    (window.bounds().size.width - px(BOARD_HORIZONTAL_PADDING)).max(px(1.0))
}

fn grid_card_width(available_width: gpui::Pixels) -> gpui::Pixels {
    ((available_width - px(GRID_COLUMN_GAP)) / GRID_COLUMN_COUNT as f32).max(px(1.0))
}

fn masonry_visible_window(scroll_handle: &ScrollHandle, window: &Window) -> (f32, f32) {
    let offset_y: f32 = scroll_handle.offset().y.into();
    let tracked_bounds = scroll_handle.bounds();
    let viewport_height: f32 = if tracked_bounds.size.height > px(0.0) {
        tracked_bounds.size.height.into()
    } else {
        window.bounds().size.height.into()
    };
    let scroll_top = (-offset_y).max(0.0);

    (
        (scroll_top - GRID_OVERSCAN_PX).max(0.0),
        scroll_top + viewport_height + GRID_OVERSCAN_PX,
    )
}

fn masonry_placement_is_visible(
    placement: &MasonryPlacement,
    visible_top: f32,
    visible_bottom: f32,
) -> bool {
    placement.top + placement.height >= visible_top && placement.top <= visible_bottom
}

#[derive(IntoElement)]
struct GridMasonry {
    state: RecordsListState,
    scroll_handle: ScrollHandle,
    available_width: gpui::Pixels,
}

impl RenderOnce for GridMasonry {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let view = self.state.view.clone();
        let card_width = grid_card_width(self.available_width);
        let estimated_heights = {
            let records = read_or_recover(&self.state.records);
            self.state
                .filtered_record_indices
                .iter()
                .filter_map(|record_index| records.get(*record_index))
                .map(estimated_grid_card_height)
                .collect::<Vec<_>>()
        };

        let layout = build_masonry_layout(
            &estimated_heights,
            GRID_COLUMN_COUNT,
            Into::<f32>::into(card_width),
            GRID_COLUMN_GAP,
            GRID_ROW_GAP,
        );

        let (visible_top, visible_bottom) = masonry_visible_window(&self.scroll_handle, window);

        let mut children = Vec::with_capacity(layout.placements.len());
        let records = read_or_recover(&self.state.records);

        for placement in layout.placements {
            let mut child = div()
                .absolute()
                .top(px(placement.top))
                .left(px(placement.left))
                .w(card_width)
                .min_w_0();

            if masonry_placement_is_visible(&placement, visible_top, visible_bottom) {
                if let Some(record_index) = self
                    .state
                    .filtered_record_indices
                    .get(placement.index)
                    .copied()
                    && let Some(record) = records.get(record_index)
                {
                    let ctx = self.state.render_context(placement.index, record);
                    child = child.child(render_list_item_with_grid_height(
                        &ctx,
                        window,
                        cx,
                        Some(placement.height),
                    ));
                } else {
                    child = child.h(px(placement.height));
                }
            } else {
                child = child.h(px(placement.height));
            }

            children.push(child.into_any_element());
        }

        div()
            .relative()
            .w_full()
            .flex_1()
            .child(
                div()
                    .id("records-grid-scroll")
                    .relative()
                    .size_full()
                    .track_scroll(&self.scroll_handle)
                    .on_scroll_wheel(move |_event, _window, cx| {
                        let _ = view.update(cx, |this, _| {
                            this.suppress_grid_auto_reveal();
                        });
                    })
                    .overflow_y_scroll()
                    .children(children),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right(px(SCROLLBAR_OVERLAY_RIGHT_OFFSET))
                    .bottom_0()
                    .child(
                        Scrollbar::vertical(&self.scroll_handle)
                            .scrollbar_show(ScrollbarShow::Scrolling),
                    ),
            )
    }
}

fn render_record_body(
    ctx: &RenderContext<'_>,
    preview_data: &PreviewData,
    styles: &ItemStyle,
    cx: &App,
) -> AnyElement {
    let compact = ctx.layout_mode == LayoutMode::Grid;
    // Click handling lives on the whole card (see `decorate_record_card`) so
    // short records have a large, stable hit area that isn't shadowed by the
    // hover-preview tooltip popover. An `id` is still required here to turn
    // the div into a stateful element so the hover tooltip can be attached.
    let mut content = div().flex_1().min_w_0().id(("record-content", ctx.index));

    if !ctx.show_preview && ctx.hover_preview_enabled {
        let preview_content_type = preview_data.content_type.clone();
        let preview_record_content = preview_data.record_content.clone();
        content = content.tooltip(move |window, cx| {
            create_preview(&preview_content_type, &preview_record_content, window, cx)
        });
    }

    if compact {
        content = content.overflow_hidden();
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

fn render_grid_record_header(ctx: &RenderContext<'_>, styles: &ItemStyle, cx: &App) -> gpui::Div {
    h_flex()
        .w_full()
        .justify_between()
        .items_start()
        .gap_2()
        .child(div().flex_1().min_w_0().child(render_record_meta(
            ctx.index,
            ctx.record,
            styles.meta_background,
            styles.badge_background,
            false,
            false,
            cx,
        )))
        .child(render_record_actions(ctx))
}

fn render_record_actions(ctx: &RenderContext<'_>) -> AnyElement {
    let compact = ctx.layout_mode == LayoutMode::Grid;
    let index = ctx.index;
    let record_id = ctx.record.id;
    let view_favorite = ctx.view.clone();
    let view_pin = ctx.view.clone();
    let view_delete = ctx.view.clone();

    // Each action button swallows its own mouse-down so the surrounding card's
    // `on_click` (which confirms the record) does not also fire when the user
    // clicks favorite / pin / delete.
    let favorite_button = {
        let button = if ctx.is_favorite {
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
        button
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_event, _window, cx| {
                view_favorite
                    .update(cx, |this, cx| {
                        this.toggle_record_favorite(record_id, cx);
                        cx.notify();
                    })
                    .ok();
            })
    };

    let pin_button = {
        let button = if ctx.record.pinned {
            Button::new(("pin-btn", index)).xsmall().primary()
        } else {
            Button::new(("pin-btn", index)).xsmall().ghost()
        };
        button
            .icon(Icon::empty().path("icon/record-pin.svg"))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
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
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
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

fn decorate_record_card(
    card: gpui::Div,
    ctx: &RenderContext<'_>,
    styles: &ItemStyle,
) -> AnyElement {
    let view_click = ctx.view.clone();
    let index = ctx.index;

    card.bg(styles.normal_background)
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
        .cursor_pointer()
        .id(("record", ctx.index))
        .on_click(move |event, window, cx| {
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
        })
        .into_any_element()
}

fn render_list_item(ctx: &RenderContext<'_>, window: &Window, cx: &mut App) -> AnyElement {
    render_list_item_with_grid_height(ctx, window, cx, None)
}

fn render_list_item_with_grid_height(
    ctx: &RenderContext<'_>,
    window: &Window,
    cx: &mut App,
    grid_height_override: Option<f32>,
) -> AnyElement {
    let compact = ctx.layout_mode == LayoutMode::Grid;
    let preview_data = PreviewData::new(ctx.record);
    let styles = ItemStyle::from_app(cx, ctx.opacity_percent);

    let card = if compact {
        let card_shell = grid_height_override.map_or_else(
            || {
                v_flex()
                    .w_full()
                    .min_h(px(GRID_CARD_MIN_HEIGHT))
                    .max_h(px(GRID_CARD_MAX_HEIGHT))
            },
            |grid_height| v_flex().w_full().h(px(grid_height)),
        );

        decorate_record_card(
            card_shell.overflow_hidden().px_2().py_1().child(
                v_flex()
                    .w_full()
                    .gap_1()
                    .child(render_grid_record_header(ctx, &styles, cx))
                    .child(render_record_body(ctx, &preview_data, &styles, cx)),
            ),
            ctx,
            &styles,
        )
    } else {
        decorate_record_card(
            v_flex().w_full().p_3().child(
                h_flex()
                    .justify_between()
                    .items_start()
                    .gap_2()
                    .child(render_record_body(ctx, &preview_data, &styles, cx))
                    .child(render_record_actions(ctx)),
            ),
            ctx,
            &styles,
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
impl RopyBoard {
    pub fn render_records_list(&self, window: &Window, context: &Context<'_, Self>) -> AnyElement {
        if self.layout_mode == LayoutMode::Grid {
            return GridMasonry {
                state: RecordsListState::from_board(self, context),
                scroll_handle: self.grid_scroll_handle.clone(),
                available_width: grid_available_width(window),
            }
            .into_any_element();
        }

        let list_state = self.list_state.clone();
        let scrollbar_state = list_state.clone();
        let state = RecordsListState::from_board(self, context);

        div()
            .relative()
            .w_full()
            .flex_1()
            .child(
                list(list_state, move |index, window, cx| {
                    state.render_row(index, window, cx)
                })
                .size_full(),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right(px(SCROLLBAR_OVERLAY_RIGHT_OFFSET))
                    .bottom_0()
                    .child(
                        Scrollbar::vertical(&scrollbar_state)
                            .scrollbar_show(ScrollbarShow::Scrolling),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};

    use super::{
        GRID_CARD_MAX_HEIGHT, GRID_CARD_MIN_HEIGHT, GRID_COLOR_SWATCH_GAP,
        GRID_COLOR_SWATCH_HEIGHT, GRID_CONTENT_PREVIEW_MAX_LINES, GRID_ESTIMATED_TEXT_LINE_HEIGHT,
        MasonryPlacement, build_masonry_layout, estimated_grid_card_height,
        estimated_grid_color_body_height, estimated_grid_text_lines, file_display_name,
        file_preview_content, list_row_for_selected_index, masonry_placement_is_visible,
        truncate_content, truncate_content_for_grid, visible_list_len,
    };
    use crate::{
        config::LayoutMode,
        repository::{ClipboardRecord, models::ContentType},
    };

    fn test_record(content: &str, content_type: ContentType) -> ClipboardRecord {
        ClipboardRecord {
            id: 1,
            content: content.to_string(),
            content_type,
            pinned: false,
            created_at: Local
                .with_ymd_and_hms(2026, 4, 18, 12, 0, 0)
                .single()
                .unwrap_or_else(|| panic!("invalid test datetime")),
            rich_text_meta: None,
        }
    }

    #[test]
    fn truncate_content_preserves_short_text() {
        assert_eq!(truncate_content("abc", 5), "abc");
    }

    #[test]
    fn truncate_content_appends_ellipsis_for_long_text() {
        assert_eq!(truncate_content("abcdef", 3), "abc...");
    }

    #[test]
    fn test_truncate_content_for_grid_allows_up_to_five_lines() {
        let content = "1\n2\n3\n4\n5";

        assert_eq!(truncate_content_for_grid(content, 120), content);
    }

    #[test]
    fn test_truncate_content_for_grid_truncates_after_five_lines() {
        let content = "1\n2\n3\n4\n5\n6";

        assert_eq!(truncate_content_for_grid(content, 120), "1\n2\n3\n4\n5...");
    }

    #[test]
    fn test_estimated_grid_text_lines_clamps_wrapped_content() {
        let content = "This is a long line that should wrap more than once in the grid card";

        assert!((2..=GRID_CONTENT_PREVIEW_MAX_LINES).contains(&estimated_grid_text_lines(content)));
    }

    #[test]
    fn test_estimated_grid_card_height_clamps_long_text_cards() {
        let record = test_record(
            "This is a very long line that should wrap repeatedly inside the grid card and reach the maximum height quickly.",
            ContentType::Text,
        );

        let height = estimated_grid_card_height(&record);

        assert!((GRID_CARD_MIN_HEIGHT..=GRID_CARD_MAX_HEIGHT).contains(&height));
    }

    #[test]
    fn test_estimated_grid_color_body_height_adds_large_swatch_space() {
        let content = "#A1B2C3";
        let expected = GRID_ESTIMATED_TEXT_LINE_HEIGHT.mul_add(
            estimated_grid_text_lines(content) as f32,
            GRID_COLOR_SWATCH_HEIGHT + GRID_COLOR_SWATCH_GAP,
        );

        assert!((estimated_grid_color_body_height(content) - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn test_estimated_grid_card_height_when_color_record_uses_taller_layout() {
        let color_record = test_record("#A1B2C3", ContentType::Text);
        let plain_record = test_record("hello", ContentType::Text);

        assert!(
            estimated_grid_card_height(&color_record) > estimated_grid_card_height(&plain_record)
        );
    }

    #[test]
    fn test_build_masonry_layout_places_items_in_shorter_column() {
        let layout = build_masonry_layout(&[100.0, 60.0, 80.0], 2, 120.0, 8.0, 8.0);

        assert_eq!(
            layout.placements,
            vec![
                MasonryPlacement {
                    index: 0,
                    column: 0,
                    left: 0.0,
                    top: 0.0,
                    height: 100.0,
                },
                MasonryPlacement {
                    index: 1,
                    column: 1,
                    left: 128.0,
                    top: 0.0,
                    height: 60.0,
                },
                MasonryPlacement {
                    index: 2,
                    column: 1,
                    left: 128.0,
                    top: 68.0,
                    height: 80.0,
                },
            ]
        );
        assert!((layout.total_height - 148.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_masonry_placement_is_visible_detects_intersection() {
        let visible = MasonryPlacement {
            index: 0,
            column: 0,
            left: 0.0,
            top: 80.0,
            height: 100.0,
        };
        let hidden = MasonryPlacement {
            index: 1,
            column: 1,
            left: 128.0,
            top: 400.0,
            height: 100.0,
        };

        assert!(masonry_placement_is_visible(&visible, 0.0, 240.0));
        assert!(!masonry_placement_is_visible(&hidden, 0.0, 240.0));
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
}

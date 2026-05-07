use std::{collections::HashSet, path::PathBuf, sync::Arc};

use gpui::{
    AnyElement, AnyView, App, Context, Window, anchored, deferred, div, img,
    prelude::{InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled},
    px,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use super::{
    super::{
        RopyBoard,
        color::{ClipboardColor, parse_clipboard_color},
        preview,
    },
    metrics::{
        GRID_CARD_MAX_HEIGHT, GRID_CARD_MIN_HEIGHT, GRID_COLOR_SWATCH_GAP,
        GRID_COLOR_SWATCH_HEIGHT, GRID_CONTENT_PREVIEW_LIMIT, GRID_CONTENT_PREVIEW_MAX_LINES,
        GRID_IMAGE_MAX_HEIGHT, LIST_COLOR_SWATCH_SIZE_PX, LIST_CONTENT_PREVIEW_LIMIT,
        LIST_CONTENT_PREVIEW_MAX_LINES, TOOLTIP_CONTENT_PREVIEW_LIMIT,
        TOOLTIP_CONTENT_PREVIEW_MAX_LINES, TruncateOptions, file_display_name,
        file_preview_content, truncate,
    },
};
use crate::{
    clipboard::thumb_path_for,
    config::LayoutMode,
    repository::{ClipboardRecord, SharedRecords, models::ContentType},
    utils::read_or_recover,
};

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
    let trimmed_content = record.content.trim_start();

    let text = if compact {
        truncate(
            trimmed_content,
            GRID_CONTENT_PREVIEW_LIMIT,
            TruncateOptions::with_max_lines(GRID_CONTENT_PREVIEW_MAX_LINES),
        )
    } else {
        truncate(
            trimmed_content,
            LIST_CONTENT_PREVIEW_LIMIT,
            TruncateOptions::with_max_lines(LIST_CONTENT_PREVIEW_MAX_LINES),
        )
    };
    let text_element = div()
        .w_full()
        .min_w_0()
        .text_color(cx.theme().secondary_foreground)
        .line_height(gpui::relative(if compact { 1.25 } else { 1.5 }))
        .child(text);
    let text_element = if compact {
        text_element.text_xs()
    } else {
        text_element.text_sm()
    };

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

fn render_file_record(cx: &App, record: &ClipboardRecord, compact: bool) -> AnyElement {
    let files = crate::utils::deserialize_file_paths(&record.content);
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
                .child(truncate(
                    &detail,
                    if compact {
                        GRID_CONTENT_PREVIEW_LIMIT
                    } else {
                        LIST_CONTENT_PREVIEW_LIMIT
                    },
                    TruncateOptions::default(),
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
        .child(Icon::empty().path("icons/filter-text.svg").size(px(12.0)))
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
        .child(Icon::empty().path("icons/filter-files.svg").size(px(12.0)))
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
            truncate(
                &file_preview_content(record_content),
                TOOLTIP_CONTENT_PREVIEW_LIMIT,
                TruncateOptions::with_max_lines(TOOLTIP_CONTENT_PREVIEW_MAX_LINES),
            ),
            window,
            cx,
        ),
        ContentType::Text | ContentType::RichText => preview::simple_tooltip(
            truncate(
                record_content,
                TOOLTIP_CONTENT_PREVIEW_LIMIT,
                TruncateOptions::with_max_lines(TOOLTIP_CONTENT_PREVIEW_MAX_LINES),
            ),
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
    fn from_app(cx: &App) -> Self {
        let theme = cx.theme();
        Self {
            selected_background: theme.accent,
            normal_background: theme.secondary,
            border: theme.border,
            hover_border: theme.foreground,
            meta_background: theme.background,
            badge_background: theme.accent,
        }
    }
}

#[expect(clippy::struct_excessive_bools)]
pub(super) struct RenderContext<'a> {
    index: usize,
    record: &'a ClipboardRecord,
    is_favorite: bool,
    is_selected: bool,
    layout_mode: LayoutMode,
    show_preview: bool,
    hover_preview_enabled: bool,
    view: &'a gpui::WeakEntity<RopyBoard>,
}

pub(super) struct RecordsListState {
    pub(super) filtered_record_indices: Arc<Vec<usize>>,
    pub(super) records: SharedRecords,
    favorite_ids: Arc<HashSet<u64>>,
    selected_index: usize,
    layout_mode: LayoutMode,
    show_preview: bool,
    hover_preview_enabled: bool,
    pub(super) view: gpui::WeakEntity<RopyBoard>,
}

impl RecordsListState {
    pub(super) fn from_board(board: &RopyBoard, context: &Context<'_, RopyBoard>) -> Self {
        Self {
            filtered_record_indices: board.filtered_record_indices.clone(),
            records: board.records.clone(),
            favorite_ids: board.favorite_ids.clone(),
            selected_index: board.selected_index,
            layout_mode: board.layout_mode,
            show_preview: board.show_preview,
            hover_preview_enabled: board.settings_editor.hover_preview_enabled
                && !board.show_clear_confirm,
            view: context.weak_entity(),
        }
    }

    pub(super) fn render_row(&self, index: usize, window: &Window, cx: &mut App) -> AnyElement {
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

    pub(super) fn render_context<'a>(
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
            view: &self.view,
        }
    }
}

fn render_record_body(
    ctx: &RenderContext<'_>,
    preview_data: &PreviewData,
    styles: &ItemStyle,
    cx: &App,
) -> AnyElement {
    let compact = ctx.layout_mode == LayoutMode::Grid;
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
        .gap_1()
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
            .icon(Icon::empty().path("icons/record-pin.svg"))
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

pub(super) fn render_list_item_with_grid_height(
    ctx: &RenderContext<'_>,
    window: &Window,
    cx: &mut App,
    grid_height_override: Option<f32>,
) -> AnyElement {
    let compact = ctx.layout_mode == LayoutMode::Grid;
    let preview_data = PreviewData::new(ctx.record);
    let styles = ItemStyle::from_app(cx);

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
            card_shell.overflow_hidden().px_1p5().py_1().child(
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

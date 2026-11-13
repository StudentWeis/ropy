use std::{collections::HashSet, path::PathBuf, sync::Arc};

use gpui_kit::{
    AnyElement, App, Context, Window,
    component::{
        ActiveTheme, Disableable, Icon, Sizable,
        button::{Button, ButtonCustomVariant, ButtonVariants},
        h_flex, v_flex,
    },
    div, img,
    prelude::{
        FluentBuilder, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
        Styled,
    },
    px,
};
use rust_i18n::t;

use super::{
    super::{
        RopyBoard,
        color::{ClipboardColor, parse_clipboard_color},
        record_ops::HistoryMutationState,
    },
    metrics::{
        GRID_CARD_MAX_HEIGHT, GRID_CARD_MIN_HEIGHT, GRID_COLOR_SWATCH_GAP,
        GRID_COLOR_SWATCH_HEIGHT, GRID_CONTENT_PREVIEW_LIMIT, GRID_CONTENT_PREVIEW_MAX_LINES,
        GRID_IMAGE_MAX_HEIGHT, LIST_COLOR_SWATCH_SIZE_PX, LIST_CONTENT_PREVIEW_LIMIT,
        LIST_CONTENT_PREVIEW_MAX_LINES, TruncateOptions, file_display_name, truncate,
    },
};
use crate::{
    config::LayoutMode,
    gui::{SharedRecords, settings::GlobalSettings},
    repository::{ClipboardRecord, ContentType, assets::thumb_path_for},
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

fn render_color_swatch(color: ClipboardColor, compact: bool, cx: &App) -> gpui_kit::Div {
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
        .line_height(gpui_kit::relative(if compact { 1.25 } else { 1.5 }))
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
    let files = crate::repository::deserialize_file_paths(&record.content);
    if files.is_empty() {
        return div()
            .text_sm()
            .text_color(cx.theme().secondary_foreground)
            .child(t!("record_file_empty"))
            .into_any_element();
    }

    let title = if files.len() == 1 {
        file_display_name(&files[0])
    } else {
        t!("record_file_count", count = files.len()).into_owned()
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
                .line_height(gpui_kit::relative(1.4))
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

fn render_rich_text_badge(badge_background: gpui_kit::Hsla, cx: &App) -> gpui_kit::Div {
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

fn render_file_badge(badge_background: gpui_kit::Hsla, cx: &App) -> gpui_kit::Div {
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

struct ItemStyle {
    selected_background: gpui_kit::Hsla,
    normal_background: gpui_kit::Hsla,
    border: gpui_kit::Hsla,
    hover_border: gpui_kit::Hsla,
    meta_background: gpui_kit::Hsla,
    badge_background: gpui_kit::Hsla,
}

impl ItemStyle {
    fn from_app(cx: &App) -> Self {
        Self {
            selected_background: cx.theme().accent,
            normal_background: cx.theme().secondary,
            border: cx.theme().border,
            hover_border: cx.theme().foreground,
            meta_background: cx.theme().background,
            badge_background: cx.theme().accent,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FavoriteState {
    Favorite,
    NotFavorite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectionState {
    Selected,
    Unselected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum HoverPreviewState {
    Enabled,
    Disabled,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RenderFlags {
    favorite: FavoriteState,
    selection: SelectionState,
    hover_preview: HoverPreviewState,
}

impl RenderFlags {
    const fn is_favorite(self) -> bool {
        matches!(self.favorite, FavoriteState::Favorite)
    }

    const fn is_selected(self) -> bool {
        matches!(self.selection, SelectionState::Selected)
    }

    const fn hover_preview_enabled(self) -> bool {
        matches!(self.hover_preview, HoverPreviewState::Enabled)
    }
}

pub(super) struct RenderContext<'a> {
    index: usize,
    record: &'a ClipboardRecord,
    flags: RenderFlags,
    layout_mode: LayoutMode,
    overlay_visible: bool,
    history_mutation: HistoryMutationState,
    view: &'a gpui_kit::WeakEntity<RopyBoard>,
}

pub(super) struct RecordsListState {
    pub(super) filtered_record_indices: Arc<Vec<usize>>,
    pub(super) records: SharedRecords,
    favorite_ids: Arc<HashSet<u64>>,
    selected_index: usize,
    layout_mode: LayoutMode,
    hover_preview_enabled: bool,
    overlay_visible: bool,
    history_mutation: HistoryMutationState,
    pub(super) view: gpui_kit::WeakEntity<RopyBoard>,
}

impl RecordsListState {
    pub(super) fn from_board(board: &RopyBoard, context: &Context<'_, RopyBoard>) -> Self {
        let overlay_visible = board.ui_state.any_overlay_visible();
        Self {
            filtered_record_indices: board.filtered_record_indices.clone(),
            records: board.records.clone(),
            favorite_ids: board.favorite_ids.clone(),
            selected_index: board.selected_index,
            layout_mode: board.layout_mode,
            hover_preview_enabled: board.settings_editor.panel_state.hover_preview_enabled
                && !overlay_visible
                && !board.is_hold_preview_active(),
            overlay_visible,
            history_mutation: board.history_mutation,
            view: context.weak_entity(),
        }
    }

    pub(super) fn render_row(&self, index: usize, window: &Window, cx: &App) -> AnyElement {
        self.render_list_row(index, window, cx)
    }

    fn render_list_row(&self, index: usize, window: &Window, cx: &App) -> AnyElement {
        let records = read_or_recover(&self.records);
        let Some(record_index) = self.filtered_record_indices.get(index).copied() else {
            return div().into_any_element();
        };
        let Some(record) = records.get(record_index) else {
            return div().into_any_element();
        };
        let item = render_list_item(&self.render_context(index, record), window, cx);
        drop(records);
        item
    }

    pub(super) fn render_context<'a>(
        &'a self,
        index: usize,
        record: &'a ClipboardRecord,
    ) -> RenderContext<'a> {
        RenderContext {
            index,
            record,
            flags: RenderFlags {
                favorite: if self.favorite_ids.contains(&record.id) {
                    FavoriteState::Favorite
                } else {
                    FavoriteState::NotFavorite
                },
                selection: if index == self.selected_index {
                    SelectionState::Selected
                } else {
                    SelectionState::Unselected
                },
                hover_preview: if self.hover_preview_enabled {
                    HoverPreviewState::Enabled
                } else {
                    HoverPreviewState::Disabled
                },
            },
            layout_mode: self.layout_mode,
            overlay_visible: self.overlay_visible,
            history_mutation: self.history_mutation,
            view: &self.view,
        }
    }
}

fn render_record_body(ctx: &RenderContext<'_>, styles: &ItemStyle, cx: &App) -> AnyElement {
    let compact = ctx.layout_mode == LayoutMode::Grid;
    let mut content = div()
        .flex_1()
        .min_w_0()
        .id(("record-content", ctx.record.id));

    if ctx.flags.hover_preview_enabled() {
        let board = ctx.view.clone();
        let record_id = ctx.record.id;
        content = content.on_hover(move |hovered, window, cx| {
            let _ = board.update(cx, |board, cx| {
                board.on_record_hover(record_id, *hovered, window, cx);
            });
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
    meta_background: gpui_kit::Hsla,
    badge_background: gpui_kit::Hsla,
    show_timestamp: bool,
    with_top_margin: bool,
    cx: &App,
) -> gpui_kit::Div {
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
    cx: &App,
) -> gpui_kit::Div {
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
        .child(render_record_actions(ctx, cx))
}

fn render_record_actions(ctx: &RenderContext<'_>, cx: &App) -> AnyElement {
    let compact = ctx.layout_mode == LayoutMode::Grid;
    let record_id = ctx.record.id;
    let view_favorite = ctx.view.clone();
    let view_pin = ctx.view.clone();
    let language = GlobalSettings::read(cx, |settings| settings.language.clone());
    let favorite_label = if ctx.flags.is_favorite() {
        t!("record_remove_favorite", locale = language.code())
    } else {
        t!("record_add_favorite", locale = language.code())
    };
    let pin_label = if ctx.record.pinned {
        t!("record_unpin", locale = language.code())
    } else {
        t!("record_pin", locale = language.code())
    };
    let action_foreground = if ctx.flags.is_selected() {
        cx.theme().accent_foreground
    } else {
        cx.theme().foreground
    };
    let action_style = ButtonCustomVariant::new(cx)
        .foreground(action_foreground)
        .hover(action_foreground.opacity(0.10))
        .active(action_foreground.opacity(0.18));

    let button = Button::new(("favorite-btn", record_id));
    let favorite_button = if ctx.flags.is_favorite() {
        button.primary().label("★")
    } else {
        button.custom(action_style).label("☆")
    }
    .xsmall()
    .w_5()
    .disabled(ctx.history_mutation.is_running())
    .accessibility_label(favorite_label.clone())
    .tooltip(favorite_label)
    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
        cx.stop_propagation();
    })
    .on_click(move |_event, _window, cx| {
        view_favorite
            .update(cx, |this, cx| {
                this.toggle_record_favorite(record_id, cx);
            })
            .ok();
    });

    let pin_button = {
        let button = if ctx.record.pinned {
            Button::new(("pin-btn", record_id)).xsmall().primary()
        } else {
            Button::new(("pin-btn", record_id))
                .xsmall()
                .custom(action_style)
        };
        button
            .icon(Icon::empty().path("icons/record-pin.svg"))
            .disabled(ctx.history_mutation.is_running())
            .accessibility_label(pin_label.clone())
            .tooltip(pin_label)
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_click(move |_event, _window, cx| {
                view_pin
                    .update(cx, |this, cx| {
                        this.toggle_record_pin(record_id, cx);
                    })
                    .ok();
            })
    };

    let delete_button = render_record_delete_button(ctx, cx);

    if compact {
        h_flex()
            .key_context("RopyCommand")
            .gap(px(2.0))
            .items_center()
            .child(favorite_button)
            .child(pin_button)
            .child(delete_button)
            .into_any_element()
    } else {
        v_flex()
            .key_context("RopyCommand")
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

fn render_record_delete_button(ctx: &RenderContext<'_>, cx: &App) -> Button {
    let record_id = ctx.record.id;
    let view = ctx.view.clone();
    let language = GlobalSettings::read(cx, |settings| settings.language.clone());
    let label = t!("record_delete", locale = language.code());
    let style = ButtonCustomVariant::new(cx)
        .foreground(cx.theme().danger)
        .hover(cx.theme().danger.opacity(0.12))
        .active(cx.theme().danger.opacity(0.20));

    Button::new(("delete-btn", record_id))
        .xsmall()
        .w_5()
        .custom(style)
        .label("×")
        .disabled(ctx.history_mutation.is_running())
        .accessibility_label(label.clone())
        .tooltip(label)
        .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
            cx.stop_propagation();
        })
        .on_click(move |_event, window, cx| {
            view.update(cx, |this, cx| {
                this.request_delete_record(record_id, window, cx);
            })
            .ok();
        })
}

fn decorate_record_card(
    card: gpui_kit::Div,
    ctx: &RenderContext<'_>,
    styles: &ItemStyle,
) -> AnyElement {
    let view_click = ctx.view.clone();
    let index = ctx.index;
    let overlay_visible = ctx.overlay_visible;

    card.bg(styles.normal_background)
        .rounded_md()
        .border_color(if ctx.flags.is_selected() {
            styles.hover_border
        } else {
            styles.border
        })
        .border_1()
        .when(!overlay_visible, |el| {
            el.hover(move |style| {
                if ctx.flags.is_selected() {
                    style
                } else {
                    style
                        .bg(styles.selected_background)
                        .border_color(styles.selected_background)
                }
            })
            .cursor_pointer()
        })
        .id(("record", ctx.record.id))
        .on_click(move |event, window, cx| {
            if overlay_visible {
                return;
            }
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

fn render_list_item(ctx: &RenderContext<'_>, window: &Window, cx: &App) -> AnyElement {
    render_list_item_with_grid_height(ctx, window, cx, None)
}

pub(super) fn render_list_item_with_grid_height(
    ctx: &RenderContext<'_>,
    _window: &Window,
    cx: &App,
    grid_height_override: Option<f32>,
) -> AnyElement {
    let compact = ctx.layout_mode == LayoutMode::Grid;
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
                    .child(render_record_body(ctx, &styles, cx)),
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
                    .child(render_record_body(ctx, &styles, cx))
                    .child(render_record_actions(ctx, cx)),
            ),
            ctx,
            &styles,
        )
    };

    let item = if compact {
        div().relative().min_w_0().child(card)
    } else {
        div().pb_2().relative().child(card)
    };

    item.into_any_element()
}

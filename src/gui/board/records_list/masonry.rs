use gpui::{
    App, RenderOnce, ScrollHandle, Window, div,
    prelude::{InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled},
    px,
};
use gpui_component::scroll::{Scrollbar, ScrollbarShow};

use super::{
    SCROLLBAR_OVERLAY_RIGHT_OFFSET,
    metrics::{GRID_COLUMN_COUNT, GRID_COLUMN_COUNT_F32, estimated_grid_card_height},
    row::{RecordsListState, render_list_item_with_grid_height},
};
use crate::{repository::SharedRecords, utils::read_or_recover};

const GRID_COLUMN_GAP: f32 = 6.0;
const GRID_ROW_GAP: f32 = 6.0;
const GRID_OVERSCAN_PX: f32 = 240.0;
const BOARD_HORIZONTAL_PADDING: f32 = 32.0;

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
        #[expect(
            clippy::cast_precision_loss,
            reason = "column index is bounded by column_count (typically 2-4)"
        )]
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

pub(super) fn grid_available_width(window: &Window) -> gpui::Pixels {
    (window.bounds().size.width - px(BOARD_HORIZONTAL_PADDING)).max(px(1.0))
}

fn grid_card_width(available_width: gpui::Pixels) -> gpui::Pixels {
    ((available_width - px(GRID_COLUMN_GAP)) / GRID_COLUMN_COUNT_F32).max(px(1.0))
}

fn masonry_visible_window(
    scroll_handle: &ScrollHandle,
    window: &Window,
    total_height: f32,
) -> (f32, f32) {
    let offset_y: f32 = scroll_handle.offset().y.into();
    let tracked_bounds = scroll_handle.bounds();
    let viewport_height: f32 = if tracked_bounds.size.height > px(0.0) {
        tracked_bounds.size.height.into()
    } else {
        window.bounds().size.height.into()
    };

    compute_masonry_visible_window(offset_y, viewport_height, total_height)
}

fn compute_masonry_visible_window(
    offset_y: f32,
    viewport_height: f32,
    total_height: f32,
) -> (f32, f32) {
    let max_scroll_top = (total_height - viewport_height).max(0.0);
    let scroll_top = (-offset_y).clamp(0.0, max_scroll_top);

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

/// Geometric description of a masonry column grid (column count + widths +
/// gaps). Lets `masonry_selected_card_fully_visible` keep a small argument
/// list while still exposing all knobs to tests.
#[derive(Debug, Clone, Copy)]
pub(super) struct MasonryColumnSpec {
    pub(super) count: usize,
    pub(super) width: f32,
    pub(super) column_gap: f32,
    pub(super) row_gap: f32,
}

/// Returns whether the selected card in `item_heights` is fully inside the
/// current viewport.
///
/// Mirrors the "skip if already visible" semantics that `ListState::scroll_to_reveal_item`
/// provides for list mode, so callers can avoid yanking the viewport back when
/// the user is already looking at the selection.
pub(super) fn masonry_selected_card_fully_visible(
    item_heights: &[f32],
    selected_index: usize,
    columns: MasonryColumnSpec,
    offset_y: f32,
    viewport_height: f32,
) -> bool {
    let layout = build_masonry_layout(
        item_heights,
        columns.count,
        columns.width,
        columns.column_gap,
        columns.row_gap,
    );
    let Some(placement) = layout.placements.get(selected_index) else {
        return true;
    };

    let max_scroll_top = (layout.total_height - viewport_height).max(0.0);
    let scroll_top = (-offset_y).clamp(0.0, max_scroll_top);
    let viewport_top = scroll_top;
    let viewport_bottom = scroll_top + viewport_height;

    placement.top >= viewport_top && placement.top + placement.height <= viewport_bottom
}

pub(in crate::gui::board) fn board_selected_card_is_visible(
    records: &SharedRecords,
    filtered_record_indices: &[usize],
    scroll_handle: &ScrollHandle,
    selected_index: usize,
) -> bool {
    if filtered_record_indices.is_empty() {
        return true;
    }

    let tracked_bounds = scroll_handle.bounds();
    let viewport_height: f32 = tracked_bounds.size.height.into();
    let viewport_width: f32 = tracked_bounds.size.width.into();
    if viewport_height <= 0.0 || viewport_width <= 0.0 {
        return true;
    }

    let card_width: f32 = Into::<f32>::into(grid_card_width(tracked_bounds.size.width));
    let item_heights = {
        let records = read_or_recover(records);
        filtered_record_indices
            .iter()
            .filter_map(|record_index| records.get(*record_index))
            .map(estimated_grid_card_height)
            .collect::<Vec<_>>()
    };

    let offset_y: f32 = scroll_handle.offset().y.into();

    masonry_selected_card_fully_visible(
        &item_heights,
        selected_index,
        MasonryColumnSpec {
            count: GRID_COLUMN_COUNT,
            width: card_width,
            column_gap: GRID_COLUMN_GAP,
            row_gap: GRID_ROW_GAP,
        },
        offset_y,
        viewport_height,
    )
}
#[derive(IntoElement)]
pub(super) struct GridMasonry {
    state: RecordsListState,
    scroll_handle: ScrollHandle,
    available_width: gpui::Pixels,
}

impl GridMasonry {
    pub(super) const fn new(
        state: RecordsListState,
        scroll_handle: ScrollHandle,
        available_width: gpui::Pixels,
    ) -> Self {
        Self {
            state,
            scroll_handle,
            available_width,
        }
    }
}

impl RenderOnce for GridMasonry {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
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

        let (visible_top, visible_bottom) =
            masonry_visible_window(&self.scroll_handle, window, layout.total_height);

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

#[cfg(test)]
mod tests {
    use super::{
        super::metrics::GRID_COLUMN_COUNT, MasonryColumnSpec, MasonryPlacement,
        build_masonry_layout, compute_masonry_visible_window, masonry_placement_is_visible,
        masonry_selected_card_fully_visible,
    };

    const fn two_col_spec(width: f32, gap: f32) -> MasonryColumnSpec {
        MasonryColumnSpec {
            count: 2,
            width,
            column_gap: gap,
            row_gap: gap,
        }
    }

    const fn one_col_spec(width: f32, gap: f32) -> MasonryColumnSpec {
        MasonryColumnSpec {
            count: 1,
            width,
            column_gap: 0.0,
            row_gap: gap,
        }
    }

    #[test]
    fn test_grid_layout_remains_two_columns() {
        assert_eq!(GRID_COLUMN_COUNT, 2);
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
    fn test_compute_visible_window_at_top() {
        let (top, bottom) = compute_masonry_visible_window(0.0, 300.0, 800.0);

        assert!((top - 0.0).abs() < f32::EPSILON);
        assert!((bottom - (300.0 + 240.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_compute_visible_window_at_bottom() {
        let total_height = 800.0;
        let viewport_height = 300.0;
        let offset_y = -(total_height - viewport_height);
        let (top, bottom) = compute_masonry_visible_window(offset_y, viewport_height, total_height);

        assert!((top - (500.0 - 240.0)).abs() < f32::EPSILON);
        assert!((bottom - (800.0 + 240.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_compute_visible_window_clamps_positive_offset_y() {
        // Positive offset_y simulates over-scroll (content pulled down).
        // Without clamping, scroll_top would become negative and the visible
        // window would flip to the top, hiding bottom records.
        let total_height = 800.0;
        let viewport_height = 300.0;
        let offset_y = 50.0;
        let (top, bottom) = compute_masonry_visible_window(offset_y, viewport_height, total_height);

        // scroll_top should be clamped to 0, not negative.
        assert!((top - 0.0).abs() < f32::EPSILON);
        assert!((bottom - (300.0 + 240.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_compute_visible_window_clamps_deep_negative_offset_y() {
        // Deep negative offset_y simulates over-scroll past the bottom.
        let total_height = 800.0;
        let viewport_height = 300.0;
        let offset_y = -700.0;
        let (top, bottom) = compute_masonry_visible_window(offset_y, viewport_height, total_height);

        // scroll_top should be clamped to max_scroll_top (500).
        assert!((top - (500.0 - 240.0)).abs() < f32::EPSILON);
        assert!((bottom - (800.0 + 240.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_selected_card_visible_when_inside_viewport() {
        // Three cards stacked across two columns; viewport covers the top.
        // Card 0 sits at top:0 height:100, card 1 at top:0 height:60 (col 1),
        // card 2 at top:108 height:80 (col 1, after row gap).
        let heights = [100.0, 60.0, 80.0];
        let visible = masonry_selected_card_fully_visible(
            &heights,
            0,
            two_col_spec(120.0, 8.0),
            /* offset_y */ 0.0,
            /* viewport_height */ 300.0,
        );
        assert!(visible);
    }

    #[test]
    fn test_selected_card_not_visible_when_user_scrolled_away() {
        // Build a long column where card 5 sits well below the visible window
        // after the user scrolled to the top. Reveal must NOT be skipped here.
        let heights = vec![200.0; 8];
        let visible = masonry_selected_card_fully_visible(
            &heights,
            5,
            one_col_spec(120.0, 8.0),
            /* offset_y */ 0.0,
            /* viewport_height */ 300.0,
        );
        assert!(!visible);
    }

    #[test]
    fn test_selected_card_visible_after_user_scrolled_to_match_selection() {
        // Same long column; user has scrolled so that card 5 is in view.
        // Card 5 in a single-column layout with height 200 + row_gap 8 starts at
        // top = 5 * (200 + 8) = 1040.
        let heights = vec![200.0; 8];
        let visible = masonry_selected_card_fully_visible(
            &heights,
            5,
            one_col_spec(120.0, 8.0),
            /* offset_y */ -1040.0,
            /* viewport_height */ 300.0,
        );
        assert!(visible);
    }

    #[test]
    fn test_selected_card_visibility_handles_empty_or_out_of_range() {
        // Out-of-range selected_index should be treated as "no reveal needed".
        let heights = [100.0_f32, 60.0];
        let visible =
            masonry_selected_card_fully_visible(&heights, 99, two_col_spec(120.0, 8.0), 0.0, 300.0);
        assert!(visible);
    }
    #[test]
    fn test_compute_visible_window_when_content_fits_viewport() {
        let total_height = 200.0;
        let viewport_height = 300.0;
        let (top, bottom) = compute_masonry_visible_window(-50.0, viewport_height, total_height);

        // max_scroll_top is 0, so scroll_top is clamped to 0 regardless of offset_y.
        assert!((top - 0.0).abs() < f32::EPSILON);
        assert!((bottom - (300.0 + 240.0)).abs() < f32::EPSILON);
    }
}

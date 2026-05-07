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
use crate::utils::read_or_recover;

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

#[cfg(test)]
mod tests {
    use super::{
        super::metrics::GRID_COLUMN_COUNT, MasonryPlacement, build_masonry_layout,
        compute_masonry_visible_window, masonry_placement_is_visible,
    };

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
    fn test_compute_visible_window_when_content_fits_viewport() {
        let total_height = 200.0;
        let viewport_height = 300.0;
        let (top, bottom) = compute_masonry_visible_window(-50.0, viewport_height, total_height);

        // max_scroll_top is 0, so scroll_top is clamped to 0 regardless of offset_y.
        assert!((top - 0.0).abs() < f32::EPSILON);
        assert!((bottom - (300.0 + 240.0)).abs() < f32::EPSILON);
    }
}

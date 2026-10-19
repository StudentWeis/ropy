use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};

use gpui_kit::{
    App, Bounds, Pixels, RenderOnce, ScrollHandle, Window,
    base::TestSupportExt,
    component::scroll::{Scrollbar, ScrollbarMode},
    div, point,
    prelude::{InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled},
    px, size,
};

use super::{
    metrics::{GRID_COLUMN_COUNT, GRID_COLUMN_COUNT_F32, estimated_grid_card_height},
    render_records_scrollbar,
    row::{RecordsListState, render_list_item_with_grid_height},
};
use crate::{gui::SharedRecords, utils::read_or_recover};

const GRID_COLUMN_GAP: f32 = 6.0;
const GRID_ROW_GAP: f32 = 6.0;
const GRID_OVERSCAN_PX: f32 = 240.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct MasonryPlacement {
    index: usize,
    column: usize,
    left: f32,
    top: f32,
    height: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::gui::board) struct MasonryLayout {
    placements: Vec<MasonryPlacement>,
    total_height: f32,
    max_height: f32,
    pub(in crate::gui::board) bounds: Vec<Bounds<Pixels>>,
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

    let bounds = placements
        .iter()
        .map(|placement| {
            Bounds::new(
                point(px(placement.left), px(placement.top)),
                size(px(column_width), px(placement.height)),
            )
        })
        .collect();
    MasonryLayout {
        placements,
        total_height,
        max_height: item_heights.iter().copied().fold(0.0, f32::max),
        bounds,
    }
}

struct CachedLayout {
    indices: Arc<Vec<usize>>,
    width: Pixels,
    rem_size: Pixels,
    layout: Arc<MasonryLayout>,
}

/// The board replaces its filtered-index Arc whenever records or filters change.
/// That identity, width and rem size own layout invalidation; scrolling does not.
#[derive(Default)]
pub(in crate::gui::board) struct MasonryState {
    geometry: Cell<(Pixels, Pixels)>,
    cached: RefCell<Option<CachedLayout>>,
}

impl MasonryState {
    pub(in crate::gui::board) fn set_geometry(&self, width: Pixels, rem_size: Pixels) {
        self.geometry.set((width, rem_size));
    }

    pub(in crate::gui::board) fn layout(
        &self,
        indices: &Arc<Vec<usize>>,
        records: &SharedRecords,
    ) -> Arc<MasonryLayout> {
        let (width, rem_size) = self.geometry.get();
        let mut cached = self.cached.borrow_mut();
        if let Some(cached) = cached.as_ref()
            && Arc::ptr_eq(&cached.indices, indices)
            && cached.width == width
            && cached.rem_size == rem_size
        {
            return cached.layout.clone();
        }
        let heights = {
            let records = read_or_recover(records);
            indices
                .iter()
                .filter_map(|ix| records.get(*ix))
                .map(estimated_grid_card_height)
                .collect::<Vec<_>>()
        };
        let layout = Arc::new(build_masonry_layout(
            &heights,
            GRID_COLUMN_COUNT,
            grid_card_width(width).into(),
            GRID_COLUMN_GAP,
            GRID_ROW_GAP,
        ));
        *cached = Some(CachedLayout {
            indices: indices.clone(),
            width,
            rem_size,
            layout: layout.clone(),
        });
        layout
    }
}

impl MasonryLayout {
    fn visible_placements(&self, top: f32, bottom: f32) -> impl Iterator<Item = &MasonryPlacement> {
        // Shortest-column placement produces monotonically increasing tops.
        let start = self
            .placements
            .partition_point(|placement| placement.top + self.max_height < top);
        self.placements[start..]
            .iter()
            .take_while(move |placement| placement.top <= bottom)
            .filter(move |placement| masonry_placement_is_visible(placement, top, bottom))
    }
}

pub(super) fn grid_available_width(window: &Window) -> Pixels {
    (window.bounds().size.width - window.rem_size() * 2.0).max(px(1.0))
}

fn grid_card_width(available_width: Pixels) -> Pixels {
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
    layout: Arc<MasonryLayout>,
}

impl GridMasonry {
    pub(super) const fn new(
        state: RecordsListState,
        scroll_handle: ScrollHandle,
        layout: Arc<MasonryLayout>,
    ) -> Self {
        Self {
            state,
            scroll_handle,
            layout,
        }
    }
}

impl RenderOnce for GridMasonry {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let layout = self.layout;
        let (visible_top, visible_bottom) =
            masonry_visible_window(&self.scroll_handle, window, layout.total_height);

        let mut children = Vec::new();
        let records = read_or_recover(&self.state.records);

        for placement in layout.visible_placements(visible_top, visible_bottom) {
            let mut child = div()
                .absolute()
                .top(px(placement.top))
                .left(px(placement.left))
                .w(layout.bounds[placement.index].size.width)
                .min_w_0();

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
            }

            children.push(child.into_any_element());
        }
        drop(records);

        div()
            .relative()
            .w_full()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .id("records-grid-scroll")
                    .test_support()
                    .relative()
                    .size_full()
                    .track_scroll(&self.scroll_handle)
                    .overflow_y_scroll()
                    .child(
                        div()
                            .relative()
                            .w_full()
                            .flex_shrink_0()
                            .h(px(layout.total_height))
                            .children(children),
                    ),
            )
            .child(render_records_scrollbar(
                Scrollbar::vertical(&self.scroll_handle).mode(ScrollbarMode::Scrolling),
            ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        super::metrics::GRID_COLUMN_COUNT, MasonryPlacement, build_masonry_layout,
        compute_masonry_visible_window, masonry_placement_is_visible,
    };

    #[test]
    fn test_masonry_visible_range_keeps_crossing_cards_and_bounds_large_history() {
        let heights = (0..10_000)
            .map(|ix| if ix % 3 == 0 { 144. } else { 96. })
            .collect::<Vec<_>>();
        let layout = build_masonry_layout(&heights, 2, 180., 6., 6.);
        for top in [0., 123., 52_000., layout.total_height - 600.] {
            let visible = layout
                .visible_placements(top, top + 600.)
                .collect::<Vec<_>>();
            assert!(
                visible.len() < 20,
                "only viewport cards should be constructed"
            );
            let expected = layout
                .placements
                .iter()
                .filter(|p| p.top <= top + 600. && p.top + p.height >= top)
                .collect::<Vec<_>>();
            assert_eq!(visible, expected);
        }
    }

    #[test]
    fn test_masonry_cache_reuses_layout_and_invalidates_snapshot_and_geometry() {
        use crate::repository::{ClipboardRecord, ContentType};
        use gpui_kit::px;
        use std::sync::{Arc, RwLock};

        let state = super::MasonryState::default();
        let records = Arc::new(RwLock::new(vec![ClipboardRecord::new(
            1,
            "short".into(),
            chrono::Local::now(),
            ContentType::Text,
        )]));
        let indices = Arc::new(vec![0]);
        state.set_geometry(px(400.), px(16.));
        let first = state.layout(&indices, &records);
        assert!(
            Arc::ptr_eq(&first, &state.layout(&indices, &records)),
            "scrolling should reuse measured geometry"
        );
        crate::utils::write_or_recover(&records)[0].content_type = ContentType::Image;
        let indices = Arc::new(vec![0]);
        let updated = state.layout(&indices, &records);
        assert!(
            updated.bounds[0].size.height > first.bounds[0].size.height,
            "same-length snapshots must invalidate old record heights"
        );
        state.set_geometry(px(600.), px(16.));
        let resized = state.layout(&indices, &records);
        assert!(resized.bounds[0].size.width > updated.bounds[0].size.width);
        state.set_geometry(px(600.), px(20.));
        assert!(
            !Arc::ptr_eq(&resized, &state.layout(&indices, &records)),
            "rem changes must invalidate layout"
        );
        assert_eq!(state.layout(&Arc::new(vec![]), &records).bounds, []);
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
    fn test_compute_visible_window_when_content_fits_viewport() {
        let total_height = 200.0;
        let viewport_height = 300.0;
        let (top, bottom) = compute_masonry_visible_window(-50.0, viewport_height, total_height);

        // max_scroll_top is 0, so scroll_top is clamped to 0 regardless of offset_y.
        assert!((top - 0.0).abs() < f32::EPSILON);
        assert!((bottom - (300.0 + 240.0)).abs() < f32::EPSILON);
    }
}

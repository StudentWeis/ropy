use gpui::{Bounds, Pixels, Point, px};

use super::{RopyBoard, records_list};
use crate::config::LayoutMode;

pub(super) const fn search_query_should_reveal_selection(
    previous_query: &str,
    next_query: &str,
) -> bool {
    previous_query.is_empty() && !next_query.is_empty()
}

impl RopyBoard {
    const fn selected_list_row(&self) -> usize {
        records_list::list_row_for_selected_index(self.selected_index, self.layout_mode)
    }

    fn reveal_grid_item(&self, index: usize) {
        if let Some(offset) = self.grid_reveal_offset(index) {
            self.grid_scroll_handle.set_offset(offset);
            return;
        }

        self.grid_scroll_handle.scroll_to_item(index);
    }

    fn grid_reveal_offset(&self, index: usize) -> Option<Point<Pixels>> {
        let viewport_bounds = self.grid_scroll_handle.bounds();
        if viewport_bounds.size.width <= px(0.0) || viewport_bounds.size.height <= px(0.0) {
            return None;
        }

        let item_bounds = self.grid_scroll_handle.bounds_for_item(index)?;
        grid_reveal_offset(viewport_bounds, item_bounds, self.grid_scroll_handle.offset())
    }

    /// Scrolls the active layout's viewport so the currently selected record is
    /// visible. Idempotent when the selection is already on screen: both
    /// `ListState::scroll_to_reveal_item` and `ScrollHandle::scroll_to_item`
    /// (with the default `FirstVisible` strategy) leave the viewport untouched
    /// when the target is already inside it.
    pub(crate) fn reveal_selected_record(&self) {
        if self.filtered_record_indices.is_empty() {
            return;
        }

        match self.layout_mode {
            LayoutMode::List => self.list_state.scroll_to_reveal_item(self.selected_list_row()),
            LayoutMode::Grid => self.reveal_grid_item(self.selected_index),
        }
    }
}

pub(super) fn grid_reveal_offset(
    viewport_bounds: Bounds<Pixels>,
    item_bounds: Bounds<Pixels>,
    current_offset: Point<Pixels>,
) -> Option<Point<Pixels>> {
    let next_x = reveal_axis_offset(
        viewport_bounds.left(),
        viewport_bounds.right(),
        item_bounds.left(),
        item_bounds.right(),
        current_offset.x,
    );
    let next_y = reveal_axis_offset(
        viewport_bounds.top(),
        viewport_bounds.bottom(),
        item_bounds.top(),
        item_bounds.bottom(),
        current_offset.y,
    );

    match (next_x, next_y) {
        (None, None) => None,
        (next_x, next_y) => Some(Point {
            x: next_x.unwrap_or(current_offset.x),
            y: next_y.unwrap_or(current_offset.y),
        }),
    }
}

fn reveal_axis_offset(
    viewport_start: Pixels,
    viewport_end: Pixels,
    item_start: Pixels,
    item_end: Pixels,
    current_offset: Pixels,
) -> Option<Pixels> {
    if item_start + current_offset < viewport_start {
        Some(viewport_start - item_start)
    } else if item_end + current_offset > viewport_end {
        Some(viewport_end - item_end)
    } else {
        None
    }
}

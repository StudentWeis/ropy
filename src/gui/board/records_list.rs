mod masonry;
mod metrics;
mod row;

use gpui::{
    AnyElement, Context, Window, div, list,
    prelude::{IntoElement, ParentElement, Styled},
    px,
};
use gpui_component::scroll::{Scrollbar, ScrollbarShow};
use masonry::{GridMasonry, grid_available_width};
pub(super) use metrics::{list_row_for_selected_index, visible_list_len};
use row::RecordsListState;

use super::RopyBoard;
use crate::config::LayoutMode;

const SCROLLBAR_OVERLAY_RIGHT_OFFSET: f32 = -10.0;

impl RopyBoard {
    pub(crate) fn render_records_list(
        &self,
        window: &Window,
        context: &Context<'_, Self>,
    ) -> AnyElement {
        if self.layout_mode == LayoutMode::Grid {
            return GridMasonry::new(
                RecordsListState::from_board(self, context),
                self.grid_scroll_handle.clone(),
                grid_available_width(window),
            )
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

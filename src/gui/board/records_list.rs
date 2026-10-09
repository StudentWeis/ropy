mod masonry;
mod metrics;
mod row;

use gpui_kit::{
    AnyElement, Context, Div, Window,
    component::scroll::{Scrollbar, ScrollbarMode},
    div, list,
    prelude::{IntoElement, ParentElement, Styled},
};
use masonry::{GridMasonry, grid_available_width};
pub(super) use metrics::{list_row_for_selected_index, visible_list_len};
use row::RecordsListState;

use super::RopyBoard;
use crate::config::LayoutMode;

fn render_records_scrollbar(scrollbar: Scrollbar) -> Div {
    // The board's trailing padding is the scrollbar gutter. Use this overlay's
    // bounds rather than the handle's bounds, which end at the card edge.
    div()
        .absolute()
        .top_0()
        .left_0()
        .right(gpui_kit::rems(-1.0))
        .bottom_0()
        .child(scrollbar.viewport_from_layout())
}

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
            .child(render_records_scrollbar(
                Scrollbar::vertical(&scrollbar_state).mode(ScrollbarMode::Scrolling),
            ))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use gpui_kit::{
        App, AppContext, Render, ScrollHandle, TestAppContext, Window, WindowOptions,
        component::scroll::{Scrollbar, ScrollbarMode},
        div, point,
        prelude::{
            InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
        },
        px,
        test::{TestSupportExt, TestWindowExt},
    };

    struct ScrollbarFixture {
        handle: ScrollHandle,
    }

    impl Render for ScrollbarFixture {
        fn render(
            &mut self,
            _: &mut Window,
            _: &mut gpui_kit::Context<'_, Self>,
        ) -> impl IntoElement {
            div().id("board").test_support().size_full().p_4().child(
                div()
                    .relative()
                    .size_full()
                    .child(
                        div()
                            .id("content")
                            .size_full()
                            .overflow_y_scroll()
                            .track_scroll(&self.handle)
                            .child(div().h(px(4000.))),
                    )
                    .child(super::render_records_scrollbar(
                        Scrollbar::vertical(&self.handle).mode(ScrollbarMode::Always),
                    )),
            )
        }
    }

    #[gpui_kit::test]
    #[expect(clippy::expect_used, reason = "headless fixture setup must succeed")]
    fn test_records_scrollbar_gutter_click_scrolls_history(cx: &mut TestAppContext) {
        let scroll_handle = ScrollHandle::default();
        let handle = scroll_handle.clone();
        let window = cx.update(|cx: &mut App| {
            gpui_kit::init(cx);
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| {
                cx.new(|_| ScrollbarFixture { handle })
            })
            .expect("open scrollbar fixture")
            .0
        });
        cx.update_window(window, |_, window, cx| {
            window.render_frame(cx);
            let bounds = window.find("board").bounds();
            window.click_at(
                "board",
                point(bounds.size.width - px(8.), bounds.size.height / 2.),
                cx,
            );
            assert!(
                scroll_handle.offset().y < px(0.),
                "clicking the trailing gutter must scroll the history"
            );
        })
        .expect("test scrollbar gutter");
    }
}

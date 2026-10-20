use gpui_kit::{
    Context,
    component::{
        ActiveTheme, Icon, Sizable,
        button::{Button, ButtonVariants},
        h_flex,
        input::Input,
    },
    div,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
    px,
};
use rust_i18n::t;

use super::RopyBoard;
pub(crate) use ropy_core::search::{ContentFilter, SearchOptions, filter_records_by_query};

/// Cycle to the adjacent `ContentFilter` in display order.
///
/// Order matches the filter buttons in the search bar header: `All → Text →
/// Image → Files → All`. `forward = true` advances; `false` walks backward.
pub(crate) const fn next_content_filter(current: ContentFilter, forward: bool) -> ContentFilter {
    const CYCLE: [ContentFilter; 4] = [
        ContentFilter::All,
        ContentFilter::Text,
        ContentFilter::Image,
        ContentFilter::Files,
    ];
    let len = CYCLE.len();
    let index = match current {
        ContentFilter::All => 0,
        ContentFilter::Text => 1,
        ContentFilter::Image => 2,
        ContentFilter::Files => 3,
    };
    let next = if forward {
        (index + 1) % len
    } else {
        (index + len - 1) % len
    };
    CYCLE[next]
}

fn create_search_option_button(
    element_id: impl Into<gpui_kit::ElementId>,
    label: impl Into<gpui_kit::SharedString>,
    is_active: bool,
    tooltip: impl Into<gpui_kit::SharedString>,
    cx: &gpui_kit::App,
) -> Button {
    let id = element_id.into();
    let tooltip = tooltip.into();
    let button = if is_active {
        let accent = cx.theme().accent;
        let variant = gpui_kit::component::button::ButtonCustomVariant::new(cx)
            .color(accent)
            .foreground(cx.theme().accent_foreground)
            .hover(accent)
            .active(accent);
        Button::new(id).custom(variant)
    } else {
        Button::new(id).ghost().opacity(0.6)
    };

    button
        .xsmall()
        .compact()
        .rounded(px(4.0))
        .text_xs()
        .label(label)
        .accessibility_label(tooltip.clone())
        .tooltip(tooltip)
}

fn create_case_sensitive_button(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> Button {
    create_search_option_button(
        "search-case-sensitive-btn",
        "Aa",
        board.filter_state.search_options.case_sensitive,
        t!("search_case_sensitive"),
        cx,
    )
    .on_click(cx.listener(|this, _, _, cx| {
        this.toggle_case_sensitive_search();
        this.sync_filtered_records_and_reveal(cx);
        cx.notify();
    }))
    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
        cx.stop_propagation();
    })
}

fn create_whole_word_button(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> Button {
    create_search_option_button(
        "search-whole-word-btn",
        "W",
        board.filter_state.search_options.whole_word,
        t!("search_match_whole_word"),
        cx,
    )
    .on_click(cx.listener(|this, _, _, cx| {
        this.toggle_whole_word_search();
        this.sync_filtered_records_and_reveal(cx);
        cx.notify();
    }))
    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
        cx.stop_propagation();
    })
}

/// Render the search icon
fn render_search_icon() -> impl IntoElement {
    div()
        .pl_2()
        .child(Icon::empty().path("icons/search.svg").size(px(16.0)))
}

/// Render the search input box
fn render_search_input_box(board: &RopyBoard) -> impl IntoElement {
    div().flex_1().min_w_0().child(
        Input::new(&board.search_input)
            .aria_label(t!("search_records"))
            .appearance(false)
            .px_1()
            .py_1(),
    )
}

/// Render the search separator
fn render_search_separator(cx: &gpui_kit::App) -> impl IntoElement {
    div().w(px(1.0)).h_3().bg(cx.theme().border).opacity(0.45)
}

/// Render the search input field with search options
fn render_search_field(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    h_flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .gap_0p5()
        .bg(cx.theme().secondary)
        .border_1()
        .border_color(cx.theme().border)
        .rounded(px(16.0))
        .p(px(2.0))
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap_0p5()
                .child(render_search_icon())
                .child(render_search_input_box(board))
                .child(render_search_separator(cx))
                .child(create_case_sensitive_button(board, cx))
                .child(create_whole_word_button(board, cx)),
        )
}

/// Render content type filter buttons (text, image, files only)
fn render_filter_buttons(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let text_filter_tooltip = t!("filter_text");
    let image_filter_tooltip = t!("filter_image");
    let files_filter_tooltip = t!("filter_files");

    let text_button = if board.filter_state.content_filter == ContentFilter::Text {
        Button::new("filter-text-btn").primary()
    } else {
        Button::new("filter-text-btn").ghost()
    };

    let image_button = if board.filter_state.content_filter == ContentFilter::Image {
        Button::new("filter-image-btn").primary()
    } else {
        Button::new("filter-image-btn").ghost()
    };

    let files_button = if board.filter_state.content_filter == ContentFilter::Files {
        Button::new("filter-files-btn").primary()
    } else {
        Button::new("filter-files-btn").ghost()
    };

    h_flex()
        .items_center()
        .bg(cx.theme().secondary)
        .border_1()
        .border_color(cx.theme().border)
        .rounded(px(16.0))
        .p(px(2.0))
        .gap(px(1.0))
        .child(
            text_button
                .icon(Icon::empty().path("icons/filter-text.svg"))
                .accessibility_label(text_filter_tooltip.clone())
                .tooltip(text_filter_tooltip)
                .rounded(px(14.0))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_content_filter(ContentFilter::Text);
                    this.sync_filtered_records_and_reveal(cx);
                    cx.notify();
                }))
                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                }),
        )
        .child(
            image_button
                .icon(Icon::empty().path("icons/filter-image.svg"))
                .accessibility_label(image_filter_tooltip.clone())
                .tooltip(image_filter_tooltip)
                .rounded(px(14.0))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_content_filter(ContentFilter::Image);
                    this.sync_filtered_records_and_reveal(cx);
                    cx.notify();
                }))
                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                }),
        )
        .child(
            files_button
                .icon(Icon::empty().path("icons/filter-files.svg"))
                .accessibility_label(files_filter_tooltip.clone())
                .tooltip(files_filter_tooltip)
                .rounded(px(14.0))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_content_filter(ContentFilter::Files);
                    this.sync_filtered_records_and_reveal(cx);
                    cx.notify();
                }))
                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                }),
        )
}

/// Render favorites filter button (separate from other filter buttons, with capsule wrapper)
fn render_favorites_button(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let favorites_filter_tooltip = t!("filter_favorites");

    let favorites_button = if board.filter_state.favorites_only {
        Button::new("filter-favorites-btn").primary()
    } else {
        Button::new("filter-favorites-btn").ghost()
    };

    h_flex()
        .items_center()
        .bg(cx.theme().secondary)
        .border_1()
        .border_color(cx.theme().border)
        .rounded(px(16.0))
        .p(px(2.0))
        .child(
            favorites_button
                .icon(Icon::empty().path("icons/filter-star.svg"))
                .accessibility_label(favorites_filter_tooltip.clone())
                .tooltip(favorites_filter_tooltip)
                .rounded(px(14.0))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_favorites_only();
                    this.sync_filtered_records_and_reveal(cx);
                    cx.notify();
                }))
                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                }),
        )
}

/// Render the search input section with content type filter buttons
pub(super) fn render_search_input(
    board: &RopyBoard,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    h_flex()
        .w_full()
        .mb_4()
        .gap_2()
        .child(render_search_field(board, cx))
        .child(render_filter_buttons(board, cx))
        .child(render_favorites_button(board, cx))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_content_filter_cycles_forward() {
        assert_eq!(
            next_content_filter(ContentFilter::All, true),
            ContentFilter::Text
        );
        assert_eq!(
            next_content_filter(ContentFilter::Text, true),
            ContentFilter::Image
        );
        assert_eq!(
            next_content_filter(ContentFilter::Image, true),
            ContentFilter::Files
        );
        assert_eq!(
            next_content_filter(ContentFilter::Files, true),
            ContentFilter::All
        );
    }

    #[test]
    fn next_content_filter_cycles_backward() {
        assert_eq!(
            next_content_filter(ContentFilter::All, false),
            ContentFilter::Files
        );
        assert_eq!(
            next_content_filter(ContentFilter::Files, false),
            ContentFilter::Image
        );
        assert_eq!(
            next_content_filter(ContentFilter::Image, false),
            ContentFilter::Text
        );
        assert_eq!(
            next_content_filter(ContentFilter::Text, false),
            ContentFilter::All
        );
    }
}

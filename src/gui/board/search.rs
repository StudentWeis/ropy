use std::{borrow::Cow, collections::HashSet};

use gpui::{
    Context, div,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
    px,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
};

use super::RopyBoard;
use crate::{
    i18n::I18n,
    repository::{ClipboardRecord, models::ContentType},
};

/// Content type filter for the clipboard history view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ContentFilter {
    /// Show all content types
    #[default]
    All,
    /// Show only text records
    Text,
    /// Show only image records
    Image,
    /// Show only favorited records
    Favorites,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchOptions {
    pub(crate) case_sensitive: bool,
    pub(crate) whole_word: bool,
}

fn is_token_char(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn has_word_boundaries(content: &str, start: usize, end: usize) -> bool {
    let previous = content[..start].chars().next_back();
    let next = content[end..].chars().next();

    let has_left_boundary = previous.is_none_or(|ch| !is_token_char(ch));
    let has_right_boundary = next.is_none_or(|ch| !is_token_char(ch));

    has_left_boundary && has_right_boundary
}

fn normalized_text(text: &str, case_sensitive: bool) -> Cow<'_, str> {
    if case_sensitive {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(text.to_lowercase())
    }
}

pub(super) fn text_matches_query(content: &str, query: &str, options: SearchOptions) -> bool {
    if query.is_empty() {
        return true;
    }

    let normalized_content = normalized_text(content, options.case_sensitive);
    let normalized_query = normalized_text(query, options.case_sensitive);

    if options.whole_word {
        normalized_content
            .match_indices(normalized_query.as_ref())
            .any(|(start, matched)| {
                let end = start + matched.len();
                has_word_boundaries(normalized_content.as_ref(), start, end)
            })
    } else {
        normalized_content.contains(normalized_query.as_ref())
    }
}

/// Filter records based on search query and content type filter
pub(super) fn filter_records_by_query(
    records: &[ClipboardRecord],
    query: &str,
    filter: ContentFilter,
    options: SearchOptions,
    favorite_ids: &HashSet<u64>,
) -> Vec<usize> {
    records
        .iter()
        .enumerate()
        .filter(|(_, record)| {
            // Apply content type filter
            let passes_type_filter = match filter {
                ContentFilter::All => true,
                ContentFilter::Text => record.content_type == ContentType::Text,
                ContentFilter::Image => record.content_type == ContentType::Image,
                ContentFilter::Favorites => favorite_ids.contains(&record.id),
            };

            if !passes_type_filter {
                return false;
            }

            // Image-only filter: ignore query entirely (images cannot be text-searched)
            if filter == ContentFilter::Image {
                return true;
            }

            // Text/All/Favorites filter: apply text search on text records
            if query.is_empty() {
                return true;
            }

            record.content_type == ContentType::Text
                && text_matches_query(&record.content, query, options)
        })
        .map(|(index, _)| index)
        .collect()
}

fn create_search_option_button(
    element_id: impl Into<gpui::ElementId>,
    label: impl Into<gpui::SharedString>,
    is_active: bool,
    tooltip: impl Into<gpui::SharedString>,
    cx: &gpui::App,
) -> Button {
    let id = element_id.into();
    let button = if is_active {
        let accent = cx.theme().accent;
        let variant = gpui_component::button::ButtonCustomVariant::new(cx)
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
        .tooltip(tooltip)
}

fn create_case_sensitive_button(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> Button {
    create_search_option_button(
        "search-case-sensitive-btn",
        "Aa",
        board.search_options.case_sensitive,
        I18n::translate(cx, "search_case_sensitive"),
        cx,
    )
    .on_click(cx.listener(|this, _, _, cx| {
        this.toggle_case_sensitive_search();
        cx.notify();
    }))
    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

fn create_whole_word_button(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> Button {
    create_search_option_button(
        "search-whole-word-btn",
        "W",
        board.search_options.whole_word,
        I18n::translate(cx, "search_match_whole_word"),
        cx,
    )
    .on_click(cx.listener(|this, _, _, cx| {
        this.toggle_whole_word_search();
        cx.notify();
    }))
    .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// Render the search icon
fn render_search_icon() -> impl IntoElement {
    div()
        .pl_2()
        .child(Icon::empty().path("icon/search.svg").size(px(16.0)))
}

/// Render the search input box
fn render_search_input_box(board: &RopyBoard) -> impl IntoElement {
    div().flex_1().min_w_0().child(
        Input::new(&board.search_input)
            .appearance(false)
            .px_1()
            .py_1(),
    )
}

/// Render the search separator
fn render_search_separator(cx: &gpui::App) -> impl IntoElement {
    div().w(px(1.0)).h_3().bg(cx.theme().border).opacity(0.45)
}

/// Render the search input field with search options
fn render_search_field(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    h_flex()
        .flex_1()
        .min_w_0()
        .items_center()
        .gap_0p5()
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

/// Render content type filter buttons
fn render_filter_buttons(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let text_filter_tooltip = I18n::translate(cx, "filter_text");
    let image_filter_tooltip = I18n::translate(cx, "filter_image");
    let favorites_filter_tooltip = I18n::translate(cx, "filter_favorites");

    let text_button = if board.content_filter == ContentFilter::Text {
        Button::new("filter-text-btn").primary()
    } else {
        Button::new("filter-text-btn").ghost()
    };

    let image_button = if board.content_filter == ContentFilter::Image {
        Button::new("filter-image-btn").primary()
    } else {
        Button::new("filter-image-btn").ghost()
    };

    let favorites_button = if board.content_filter == ContentFilter::Favorites {
        Button::new("filter-favorites-btn").primary()
    } else {
        Button::new("filter-favorites-btn").ghost()
    };

    h_flex()
        .items_center()
        .border_1()
        .border_color(cx.theme().border)
        .rounded(px(16.0))
        .p(px(2.0))
        .gap(px(1.0))
        .child(
            text_button
                .icon(Icon::empty().path("icon/filter-text.svg"))
                .tooltip(text_filter_tooltip)
                .rounded(px(14.0))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_content_filter(ContentFilter::Text);
                    cx.notify();
                }))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
        )
        .child(
            image_button
                .icon(Icon::empty().path("icon/filter-image.svg"))
                .tooltip(image_filter_tooltip)
                .rounded(px(14.0))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_content_filter(ContentFilter::Image);
                    cx.notify();
                }))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
        )
        .child(
            favorites_button
                .icon(Icon::empty().path("icon/filter-star.svg"))
                .tooltip(favorites_filter_tooltip)
                .rounded(px(14.0))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_content_filter(ContentFilter::Favorites);
                    cx.notify();
                }))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::{ClipboardRecord, models::ContentType};

    fn empty_favorites() -> HashSet<u64> {
        HashSet::new()
    }

    /// Helper: build a mixed set of test records (2 text + 1 image)
    fn mixed_records() -> Vec<ClipboardRecord> {
        vec![
            ClipboardRecord {
                id: 1,
                content: "Hello World".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "Goodbye World".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 3,
                content: "image_data".to_string(),
                content_type: ContentType::Image,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ]
    }

    // --- ContentFilter::All (default) tests ---

    #[test]
    fn test_filter_all_no_query_returns_everything() {
        let records = mixed_records();
        let result = filter_records_by_query(
            &records,
            "",
            ContentFilter::All,
            SearchOptions::default(),
            &empty_favorites(),
        );
        assert_eq!(result, vec![0, 1, 2]);
    }

    #[test]
    fn test_filter_all_with_query_matches_text_only() {
        let records = mixed_records();
        let result = filter_records_by_query(
            &records,
            "world",
            ContentFilter::All,
            SearchOptions::default(),
            &empty_favorites(),
        );
        assert_eq!(result, vec![0, 1]);
    }

    #[test]
    fn test_search_contains_case_insensitive_matches_all_variants() {
        let records = vec![
            ClipboardRecord {
                id: 1,
                content: "Hello World".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "HELLO world".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ];

        let result = filter_records_by_query(
            &records,
            "hello",
            ContentFilter::All,
            SearchOptions::default(),
            &empty_favorites(),
        );
        assert_eq!(result, vec![0, 1]);
    }

    #[test]
    fn test_search_contains_case_sensitive_matches_only_same_case() {
        let records = vec![
            ClipboardRecord {
                id: 1,
                content: "Hello World".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "hello world".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ];

        let result = filter_records_by_query(
            &records,
            "Hello",
            ContentFilter::All,
            SearchOptions {
                case_sensitive: true,
                whole_word: false,
            },
            &empty_favorites(),
        );

        assert_eq!(result, vec![0]);
    }

    #[test]
    fn test_search_whole_word_case_insensitive_matches_token_boundaries() {
        let records = vec![
            ClipboardRecord {
                id: 1,
                content: "Say hello world".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "say HELLO again".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 3,
                content: "shelloworld".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ];

        let result = filter_records_by_query(
            &records,
            "hello",
            ContentFilter::All,
            SearchOptions {
                case_sensitive: false,
                whole_word: true,
            },
            &empty_favorites(),
        );

        assert_eq!(result, vec![0, 1]);
    }

    #[test]
    fn test_search_whole_word_case_sensitive_rejects_case_mismatch() {
        let records = vec![
            ClipboardRecord {
                id: 1,
                content: "say Hello again".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "say hello again".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
        ];

        let result = filter_records_by_query(
            &records,
            "Hello",
            ContentFilter::All,
            SearchOptions {
                case_sensitive: true,
                whole_word: true,
            },
            &empty_favorites(),
        );

        assert_eq!(result, vec![0]);
    }

    #[test]
    fn test_search_whole_word_partial_token_returns_no_match() {
        let records = vec![ClipboardRecord {
            id: 1,
            content: "hello_world hello2".to_string(),
            content_type: ContentType::Text,
            created_at: chrono::Local::now(),
            pinned: false,
        }];

        let result = filter_records_by_query(
            &records,
            "hello",
            ContentFilter::All,
            SearchOptions {
                case_sensitive: false,
                whole_word: true,
            },
            &empty_favorites(),
        );

        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_all_with_query_no_matches() {
        let records = vec![ClipboardRecord {
            id: 1,
            content: "Hello".to_string(),
            content_type: ContentType::Text,
            created_at: chrono::Local::now(),
            pinned: false,
        }];

        let result = filter_records_by_query(
            &records,
            "xyz",
            ContentFilter::All,
            SearchOptions::default(),
            &empty_favorites(),
        );
        assert!(result.is_empty());
    }

    #[test]
    fn test_filter_all_with_query_excludes_image() {
        let records = vec![ClipboardRecord {
            id: 1,
            content: "Image content".to_string(),
            content_type: ContentType::Image,
            created_at: chrono::Local::now(),
            pinned: false,
        }];

        let result = filter_records_by_query(
            &records,
            "image",
            ContentFilter::All,
            SearchOptions::default(),
            &empty_favorites(),
        );
        assert!(result.is_empty());
    }

    // --- ContentFilter::Text tests ---

    #[test]
    fn test_filter_text_no_query_returns_text_only() {
        let records = mixed_records();
        let result = filter_records_by_query(
            &records,
            "",
            ContentFilter::Text,
            SearchOptions::default(),
            &empty_favorites(),
        );
        assert_eq!(result, vec![0, 1]);
    }

    #[test]
    fn test_filter_text_with_query_matches_within_text() {
        let records = mixed_records();
        let result = filter_records_by_query(
            &records,
            "hello",
            ContentFilter::Text,
            SearchOptions::default(),
            &empty_favorites(),
        );
        assert_eq!(result, vec![0]);
    }

    // --- ContentFilter::Image tests ---

    #[test]
    fn test_filter_image_no_query_returns_images_only() {
        let records = mixed_records();
        let result = filter_records_by_query(
            &records,
            "",
            ContentFilter::Image,
            SearchOptions::default(),
            &empty_favorites(),
        );
        assert_eq!(result, vec![2]);
    }

    #[test]
    fn test_filter_image_with_query_ignores_query_for_all_search_modes() {
        let records = mixed_records();
        let no_favorites = empty_favorites();
        for options in [
            SearchOptions::default(),
            SearchOptions {
                case_sensitive: false,
                whole_word: true,
            },
        ] {
            let result = filter_records_by_query(
                &records,
                "nonexistent",
                ContentFilter::Image,
                options,
                &no_favorites,
            );
            assert_eq!(result, vec![2]);
        }
    }

    // --- ContentFilter::Favorites tests ---

    #[test]
    fn test_filter_favorites_no_query_returns_favorited_only() {
        let records = mixed_records();
        let favorites: HashSet<u64> = [1, 3].into_iter().collect();
        let result = filter_records_by_query(
            &records,
            "",
            ContentFilter::Favorites,
            SearchOptions::default(),
            &favorites,
        );
        assert_eq!(result, vec![0, 2]);
    }

    #[test]
    fn test_filter_favorites_with_query_searches_text_favorites() {
        let records = mixed_records();
        let favorites: HashSet<u64> = [1, 2].into_iter().collect();
        let result = filter_records_by_query(
            &records,
            "hello",
            ContentFilter::Favorites,
            SearchOptions::default(),
            &favorites,
        );
        assert_eq!(result, vec![0]);
    }

    #[test]
    fn test_filter_favorites_empty_set_returns_nothing() {
        let records = mixed_records();
        let result = filter_records_by_query(
            &records,
            "",
            ContentFilter::Favorites,
            SearchOptions::default(),
            &empty_favorites(),
        );
        assert!(result.is_empty());
    }
}

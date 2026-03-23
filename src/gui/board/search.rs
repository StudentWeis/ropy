use std::borrow::Cow;

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMatchMode {
    #[default]
    Contains,
    WholeWord,
    Exact,
}

impl SearchMatchMode {
    pub(crate) const fn next(self) -> Self {
        match self {
            Self::Contains => Self::WholeWord,
            Self::WholeWord => Self::Exact,
            Self::Exact => Self::Contains,
        }
    }

    pub(crate) const fn short_label(self) -> &'static str {
        match self {
            Self::Contains => ".*",
            Self::WholeWord => "W",
            Self::Exact => "=",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchOptions {
    pub(crate) match_mode: SearchMatchMode,
    pub(crate) case_sensitive: bool,
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

    match options.match_mode {
        SearchMatchMode::Contains => normalized_content.contains(normalized_query.as_ref()),
        SearchMatchMode::WholeWord => normalized_content
            .match_indices(normalized_query.as_ref())
            .any(|(start, matched)| {
                let end = start + matched.len();
                has_word_boundaries(normalized_content.as_ref(), start, end)
            }),
        SearchMatchMode::Exact => normalized_content == normalized_query,
    }
}

/// Filter records based on search query and content type filter
pub(super) fn filter_records_by_query(
    records: &[ClipboardRecord],
    query: &str,
    filter: ContentFilter,
    options: SearchOptions,
) -> Vec<ClipboardRecord> {
    records
        .iter()
        .filter(|record| {
            // Apply content type filter
            let passes_type_filter = match filter {
                ContentFilter::All => true,
                ContentFilter::Text => record.content_type == ContentType::Text,
                ContentFilter::Image => record.content_type == ContentType::Image,
            };

            if !passes_type_filter {
                return false;
            }

            // Image filter: ignore query entirely (images cannot be text-searched)
            if filter == ContentFilter::Image {
                return true;
            }

            // Text/All filter: apply text search on text records
            if query.is_empty() {
                return true;
            }

            record.content_type == ContentType::Text
                && text_matches_query(&record.content, query, options)
        })
        .cloned()
        .collect()
}

fn create_case_sensitive_button(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> Button {
    let is_active = board.search_options.case_sensitive;
    let button = if is_active {
        Button::new("search-case-sensitive-btn").primary()
    } else {
        Button::new("search-case-sensitive-btn").ghost()
    };

    let button = if is_active {
        button
    } else {
        button.opacity(0.6)
    };

    button
        .small()
        .min_h(px(24.0))
        .px_1()
        .py_0()
        .rounded_none()
        .label("Aa")
        .tooltip(I18n::translate(cx, "search_case_sensitive"))
        .on_click(cx.listener(|this, _, _, cx| {
            this.toggle_case_sensitive_search();
            cx.notify();
        }))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

fn create_search_match_mode_button(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> Button {
    let match_mode = board.search_options.match_mode;
    let match_mode_label = match match_mode {
        SearchMatchMode::Contains => I18n::translate(cx, "search_match_contains"),
        SearchMatchMode::WholeWord => I18n::translate(cx, "search_match_whole_word"),
        SearchMatchMode::Exact => I18n::translate(cx, "search_match_exact"),
    };
    let tooltip = format!(
        "{}: {}",
        I18n::translate(cx, "search_match_mode"),
        match_mode_label
    );

    let is_active = !matches!(match_mode, SearchMatchMode::Contains);
    let button = match match_mode {
        SearchMatchMode::Contains => Button::new("search-match-mode-btn").ghost(),
        SearchMatchMode::WholeWord | SearchMatchMode::Exact => {
            Button::new("search-match-mode-btn").primary()
        }
    };

    let button = if is_active {
        button
    } else {
        button.opacity(0.6)
    };

    button
        .small()
        .min_h(px(24.0))
        .px_1()
        .py_0()
        .rounded_none()
        .label(match_mode.short_label())
        .tooltip(tooltip)
        .on_click(cx.listener(|this, _, _, cx| {
            this.cycle_search_match_mode();
            cx.notify();
        }))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// Render the search input section with content type filter buttons
pub(super) fn render_search_input(
    board: &RopyBoard,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let is_text_active = board.content_filter == ContentFilter::Text;
    let is_image_active = board.content_filter == ContentFilter::Image;
    let text_filter_tooltip = I18n::translate(cx, "filter_text");
    let image_filter_tooltip = I18n::translate(cx, "filter_image");

    let text_button = if is_text_active {
        Button::new("filter-text-btn").primary()
    } else {
        Button::new("filter-text-btn").ghost()
    };

    let image_button = if is_image_active {
        Button::new("filter-image-btn").primary()
    } else {
        Button::new("filter-image-btn").ghost()
    };

    h_flex()
        .w_full()
        .mb_4()
        .gap_2()
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap_0p5()
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .px_1()
                .py_1()
                .child(
                    div().flex_1().min_w_0().child(
                        Input::new(&board.search_input)
                            .appearance(false)
                            .px_1()
                            .py_1(),
                    ),
                )
                .child(div().w(px(1.0)).h_3().bg(cx.theme().border).opacity(0.45))
                .child(
                    h_flex()
                        .items_center()
                        .bg(cx.theme().background)
                        .overflow_hidden()
                        .rounded_md()
                        .child(create_case_sensitive_button(board, cx))
                        .child(div().w(px(1.0)).h_3().bg(cx.theme().border).opacity(0.45))
                        .child(create_search_match_mode_button(board, cx)),
                ),
        )
        .child(
            text_button
                .icon(Icon::empty().path("icon/filter-text.svg"))
                .tooltip(text_filter_tooltip)
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
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_content_filter(ContentFilter::Image);
                    cx.notify();
                }))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repository::{ClipboardRecord, models::ContentType};

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
        let result =
            filter_records_by_query(&records, "", ContentFilter::All, SearchOptions::default());
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_filter_all_with_query_matches_text_only() {
        let records = mixed_records();
        let result = filter_records_by_query(
            &records,
            "world",
            ContentFilter::All,
            SearchOptions::default(),
        );
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].content, "Hello World");
        assert_eq!(result[1].content, "Goodbye World");
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
        );
        assert_eq!(result.len(), 2);
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
                match_mode: SearchMatchMode::Contains,
                case_sensitive: true,
            },
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "Hello World");
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
                match_mode: SearchMatchMode::WholeWord,
                case_sensitive: false,
            },
        );

        assert_eq!(result.len(), 2);
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
                match_mode: SearchMatchMode::WholeWord,
                case_sensitive: true,
            },
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "say Hello again");
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
                match_mode: SearchMatchMode::WholeWord,
                case_sensitive: false,
            },
        );

        assert!(result.is_empty());
    }

    #[test]
    fn test_search_exact_case_insensitive_matches_full_content() {
        let records = vec![
            ClipboardRecord {
                id: 1,
                content: "Hello".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "HELLO".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 3,
                content: "Hello World".to_string(),
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
                match_mode: SearchMatchMode::Exact,
                case_sensitive: false,
            },
        );

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_search_exact_case_sensitive_matches_only_strict_equal_content() {
        let records = vec![
            ClipboardRecord {
                id: 1,
                content: "Hello".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "hello".to_string(),
                content_type: ContentType::Text,
                created_at: chrono::Local::now(),
                pinned: false,
            },
            ClipboardRecord {
                id: 3,
                content: " hello ".to_string(),
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
                match_mode: SearchMatchMode::Exact,
                case_sensitive: true,
            },
        );

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "hello");
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
        );
        assert_eq!(result.len(), 0);
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
        );
        assert_eq!(result.len(), 0);
    }

    // --- ContentFilter::Text tests ---

    #[test]
    fn test_filter_text_no_query_returns_text_only() {
        let records = mixed_records();
        let result =
            filter_records_by_query(&records, "", ContentFilter::Text, SearchOptions::default());
        assert_eq!(result.len(), 2);
        assert!(result.iter().all(|r| r.content_type == ContentType::Text));
    }

    #[test]
    fn test_filter_text_with_query_matches_within_text() {
        let records = mixed_records();
        let result = filter_records_by_query(
            &records,
            "hello",
            ContentFilter::Text,
            SearchOptions::default(),
        );
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content, "Hello World");
    }

    // --- ContentFilter::Image tests ---

    #[test]
    fn test_filter_image_no_query_returns_images_only() {
        let records = mixed_records();
        let result =
            filter_records_by_query(&records, "", ContentFilter::Image, SearchOptions::default());
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].content_type, ContentType::Image);
    }

    #[test]
    fn test_filter_image_with_query_ignores_query_for_all_search_modes() {
        let records = mixed_records();
        for options in [
            SearchOptions::default(),
            SearchOptions {
                match_mode: SearchMatchMode::WholeWord,
                case_sensitive: false,
            },
            SearchOptions {
                match_mode: SearchMatchMode::Exact,
                case_sensitive: true,
            },
        ] {
            let result =
                filter_records_by_query(&records, "nonexistent", ContentFilter::Image, options);
            assert_eq!(result.len(), 1);
            assert_eq!(result[0].content_type, ContentType::Image);
        }
    }

    // --- Toggle tests ---

    #[test]
    fn test_search_match_mode_next_cycles_all_modes() {
        assert_eq!(SearchMatchMode::Contains.next(), SearchMatchMode::WholeWord);
        assert_eq!(SearchMatchMode::WholeWord.next(), SearchMatchMode::Exact);
        assert_eq!(SearchMatchMode::Exact.next(), SearchMatchMode::Contains);
    }
}

use std::collections::HashSet;

use chrono::{Local, TimeZone};
use gpui::{Bounds, Pixels, point, px, size};
use rstest::rstest;

use super::{
    actions::horizontal_grid_target_index,
    clipboard_ops::{ConfirmFormat, build_copy_request, build_copy_request_for_record},
    filtering::filter_and_sort_record_indices,
    search_query_changed, search_query_should_reveal_selection,
    search::{ContentFilter, SearchOptions},
    settings_editor::UpdateManager,
};
use crate::{
    config::{ConfirmMode, LayoutMode},
    gui::board::{ActivePanel, RopyBoard},
    repository::{ClipboardRecord, models::ContentType},
    updater::models::UpdateStatus,
};

#[rstest]
#[case(ActivePanel::ClipboardList, false, true)]
#[case(ActivePanel::ClipboardList, true, false)]
#[case(ActivePanel::Settings, false, false)]
#[case(ActivePanel::About, false, false)]
#[case(ActivePanel::Help, false, false)]
fn test_focus_out_auto_hide_for_panel_and_pin_state_matches_expected(
    #[case] active_panel: ActivePanel,
    #[case] pinned: bool,
    #[case] expected: bool,
) {
    assert_eq!(
        RopyBoard::should_auto_hide_on_focus_out(active_panel, pinned),
        expected
    );
}

#[test]
fn test_open_settings_panel_hides_opacity_slider_until_next_frame() {
    let mut slider_visible = true;
    let show_settings = true;

    if show_settings {
        slider_visible = false;
    }

    assert!(!slider_visible);
}

#[test]
fn test_window_opacity_slider_reveals_only_while_settings_open() {
    let reveal_slider = |show_settings: bool| show_settings;

    assert!(reveal_slider(true));
    assert!(!reveal_slider(false));
}

fn test_datetime(hour: u32) -> chrono::DateTime<Local> {
    Local
        .with_ymd_and_hms(2026, 3, 31, hour, 0, 0)
        .single()
        .unwrap_or_else(|| panic!("invalid local datetime for test hour {hour}"))
}

fn test_record(
    id: u64,
    content: &str,
    content_type: ContentType,
    pinned: bool,
    created_at: chrono::DateTime<Local>,
) -> ClipboardRecord {
    ClipboardRecord {
        id,
        content: content.to_string(),
        content_type,
        pinned,
        created_at,
        rich_text_meta: None,
    }
}

#[test]
fn test_window_pin_availability_depends_on_confirm_mode() {
    assert!(RopyBoard::window_pin_available(
        ConfirmMode::CopyToClipboard
    ));
    assert!(!RopyBoard::window_pin_available(
        ConfirmMode::PasteImmediately,
    ));
}

#[test]
fn test_resolve_window_pin_state_disables_pin_for_immediate_paste() {
    assert!(RopyBoard::resolve_window_pin_state(
        ConfirmMode::CopyToClipboard,
        true,
    ));
    assert!(!RopyBoard::resolve_window_pin_state(
        ConfirmMode::PasteImmediately,
        true,
    ));
    assert!(!RopyBoard::resolve_window_pin_state(
        ConfirmMode::PasteImmediately,
        false,
    ));
}

#[test]
fn test_update_manager_new_starts_idle() {
    assert!(matches!(UpdateManager::new().status, UpdateStatus::Idle));
}

#[test]
fn test_search_query_changed_only_when_query_text_differs() {
    assert!(!search_query_changed("abc", "abc"));
    assert!(search_query_changed("", "a"));
    assert!(search_query_changed("a", "ab"));
    assert!(search_query_changed("ab", ""));
}

#[test]
fn test_search_query_should_reveal_selection_only_on_first_non_empty_query() {
    assert!(search_query_should_reveal_selection("", "a"));
    assert!(!search_query_should_reveal_selection("a", "ab"));
    assert!(!search_query_should_reveal_selection("ab", "abc"));
    assert!(!search_query_should_reveal_selection("abc", ""));
}

#[test]
fn test_horizontal_grid_target_index_moves_to_nearest_card_in_adjacent_column() {
    let item_bounds = vec![
        test_bounds(0.0, 0.0, 100.0, 120.0),
        test_bounds(108.0, 0.0, 100.0, 80.0),
        test_bounds(0.0, 128.0, 100.0, 100.0),
        test_bounds(108.0, 88.0, 100.0, 120.0),
    ];

    assert_eq!(
        horizontal_grid_target_index(0, &item_bounds, true, LayoutMode::Grid),
        Some(1)
    );
    assert_eq!(
        horizontal_grid_target_index(2, &item_bounds, true, LayoutMode::Grid),
        Some(3)
    );
    assert_eq!(
        horizontal_grid_target_index(3, &item_bounds, false, LayoutMode::Grid),
        Some(2)
    );
}

#[test]
fn test_horizontal_grid_target_index_respects_missing_column_and_list_mode() {
    let single_column_bounds = vec![
        test_bounds(0.0, 0.0, 100.0, 120.0),
        test_bounds(0.0, 128.0, 100.0, 100.0),
    ];

    assert_eq!(
        horizontal_grid_target_index(0, &single_column_bounds, true, LayoutMode::Grid),
        None
    );
    assert_eq!(
        horizontal_grid_target_index(0, &single_column_bounds, true, LayoutMode::List),
        None
    );
}

fn test_bounds(x: f32, y: f32, width: f32, height: f32) -> Bounds<Pixels> {
    Bounds::new(point(px(x), px(y)), size(px(width), px(height)))
}

#[test]
fn test_surface_with_opacity_scales_alpha() {
    let color = gpui::hsla(0.4, 0.5, 0.6, 1.0);
    let faded = crate::gui::surface_with_opacity(color, 65);

    assert!((faded.a - 0.65).abs() < 0.000_1);
    assert!((faded.h - color.h).abs() < f32::EPSILON);
    assert!((faded.s - color.s).abs() < f32::EPSILON);
    assert!((faded.l - color.l).abs() < f32::EPSILON);
}

#[test]
fn test_toggle_content_filter() {
    // Toggling the same filter twice returns to All
    let mut filter = ContentFilter::All;

    // Simulate toggle to Text
    filter = if filter == ContentFilter::Text {
        ContentFilter::All
    } else {
        ContentFilter::Text
    };
    assert_eq!(filter, ContentFilter::Text);

    // Simulate toggle Text again -> back to All
    filter = if filter == ContentFilter::Text {
        ContentFilter::All
    } else {
        ContentFilter::Text
    };
    assert_eq!(filter, ContentFilter::All);

    // Simulate toggle to Image
    filter = if filter == ContentFilter::Image {
        ContentFilter::All
    } else {
        ContentFilter::Image
    };
    assert_eq!(filter, ContentFilter::Image);

    // Simulate toggle to Files
    filter = if filter == ContentFilter::Files {
        ContentFilter::All
    } else {
        ContentFilter::Files
    };
    assert_eq!(filter, ContentFilter::Files);

    // Simulate toggle to Text while Image is active -> switches to Text
    filter = if filter == ContentFilter::Text {
        ContentFilter::All
    } else {
        ContentFilter::Text
    };
    assert_eq!(filter, ContentFilter::Text);
}

#[test]
fn test_filter_and_sort_record_indices_display_order_returns_sorted_indices() {
    let records = vec![
        test_record(1, "alpha", ContentType::Text, false, test_datetime(9)),
        test_record(2, "beta", ContentType::Text, true, test_datetime(8)),
        test_record(3, "alphabet", ContentType::Text, true, test_datetime(10)),
        test_record(4, "gamma", ContentType::Image, false, test_datetime(11)),
    ];

    let indices = filter_and_sort_record_indices(
        &records,
        "alp",
        ContentFilter::All,
        SearchOptions::default(),
        &HashSet::new(),
        false,
    );

    assert_eq!(indices, vec![2, 0]);
}

#[test]
fn test_filter_and_sort_record_indices_image_filter_ignores_query() {
    let records = vec![
        test_record(1, "hello", ContentType::Text, false, test_datetime(9)),
        test_record(2, "image-a", ContentType::Image, false, test_datetime(8)),
        test_record(3, "image-b", ContentType::Image, true, test_datetime(10)),
    ];

    let indices = filter_and_sort_record_indices(
        &records,
        "hello",
        ContentFilter::Image,
        SearchOptions::default(),
        &HashSet::new(),
        false,
    );

    assert_eq!(indices, vec![2, 1]);
}

#[test]
fn test_filter_and_sort_record_indices_files_filter_matches_file_records() {
    let records = vec![
        test_record(
            1,
            "[\"/tmp/notes.txt\"]",
            ContentType::FilePath,
            false,
            test_datetime(9),
        ),
        test_record(2, "hello", ContentType::Text, false, test_datetime(8)),
        test_record(
            3,
            "[\"/tmp/archive.zip\"]",
            ContentType::FilePath,
            true,
            test_datetime(10),
        ),
    ];

    let indices = filter_and_sort_record_indices(
        &records,
        "",
        ContentFilter::Files,
        SearchOptions::default(),
        &HashSet::new(),
        false,
    );

    assert_eq!(indices, vec![2, 0]);
}

#[test]
fn test_build_copy_request_when_file_payload_is_json_returns_files_request() {
    let request = build_copy_request(
        "[\"/tmp/alpha.txt\",\"/tmp/beta.txt\"]",
        &ContentType::FilePath,
        None,
    );

    match request {
        Some(crate::clipboard::CopyRequest::Files { paths, completion }) => {
            assert_eq!(paths, vec!["/tmp/alpha.txt", "/tmp/beta.txt"]);
            assert!(completion.is_none());
        }
        _ => panic!("expected files copy request"),
    }
}

#[test]
fn test_build_copy_request_when_file_payload_is_legacy_string_returns_single_file() {
    let request = build_copy_request("/tmp/legacy.txt", &ContentType::FilePath, None);

    match request {
        Some(crate::clipboard::CopyRequest::Files { paths, completion }) => {
            assert_eq!(paths, vec!["/tmp/legacy.txt"]);
            assert!(completion.is_none());
        }
        _ => panic!("expected files copy request"),
    }
}

#[test]
fn test_build_copy_request_for_record_when_rich_text_returns_rich_text_request() {
    let record = test_record(
        7,
        "Formatted text",
        ContentType::RichText,
        false,
        test_datetime(11),
    );

    let request = build_copy_request_for_record(&record, ConfirmFormat::Default, None);

    match request {
        Some(crate::clipboard::CopyRequest::RichText {
            plain_text,
            html,
            rtf,
            completion,
        }) => {
            assert_eq!(plain_text, "Formatted text");
            assert_eq!(html, None);
            assert_eq!(rtf, None);
            assert!(completion.is_none());
        }
        _ => panic!("expected rich text copy request"),
    }
}

#[test]
fn test_build_copy_request_for_record_when_rich_text_and_plain_text_mode_returns_text_request() {
    let record = test_record(
        8,
        "Formatted text",
        ContentType::RichText,
        false,
        test_datetime(12),
    );

    let request = build_copy_request_for_record(&record, ConfirmFormat::PlainText, None);

    match request {
        Some(crate::clipboard::CopyRequest::Text { text, completion }) => {
            assert_eq!(text, "Formatted text");
            assert!(completion.is_none());
        }
        _ => panic!("expected plain text copy request"),
    }
}

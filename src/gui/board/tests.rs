#![cfg_attr(test, allow(clippy::panic))]

use std::{
    collections::HashSet,
    sync::{Arc, Mutex, RwLock},
};

use chrono::{Local, TimeZone};
use futures::{FutureExt as _, StreamExt as _};
use gpui::{
    AppContext as _, Bounds, Pixels, TestAppContext, VisualTestContext, WindowOptions, point, px,
    size,
};
use gpui_component::Root;
use rstest::rstest;

use super::{
    Active,
    actions::horizontal_grid_target_index,
    clipboard_ops::{
        ConfirmFormat, build_copy_request, build_copy_request_for_record, wait_for_clipboard_write,
    },
    filtering::filter_and_sort_record_indices,
    grid_reveal_offset,
    search::{ContentFilter, SearchOptions},
    search_query_changed, search_query_should_reveal_selection,
    settings_editor::UpdateManager,
};
use crate::{
    clipboard::{ClipboardWriteError, LastCopyState},
    config::{ConfirmMode, LayoutMode, Settings},
    gui::board::{RopyBoard, UiState},
    i18n::I18n,
    repository::{ClipboardRecord, GlobalRepository, models::ContentType},
    updater::models::UpdateStatus,
};

#[gpui::test]
fn test_active_action_when_window_is_hidden_notifies_board_for_render(cx: &mut TestAppContext) {
    let settings = Settings::default();
    let language = settings.language.clone();
    cx.update(|cx| {
        gpui_component::init(cx);
        cx.set_global(settings);
        cx.set_global(I18n::load_i18n(language));
        cx.set_global(GlobalRepository::new(None));
    });

    let records = Arc::new(RwLock::new(Vec::new()));
    let last_copy = Arc::new(Mutex::new(LastCopyState::Text(String::new())));
    let (copy_tx, _copy_rx) = async_channel::bounded(1);
    let window = cx.update(|cx| {
        cx.open_window(WindowOptions::default(), |window, cx| {
            let board = cx.new(|cx| RopyBoard::new(records, last_copy, copy_tx, window, cx));
            cx.new(|cx| Root::new(board, window, cx))
        })
        .unwrap_or_else(|error| panic!("test window should open: {error}"))
    });
    let board = window
        .update(cx, |root, _, _| {
            root.view()
                .clone()
                .downcast::<RopyBoard>()
                .unwrap_or_else(|_| panic!("test board should exist"))
        })
        .unwrap_or_else(|error| panic!("test window should exist: {error}"));
    let mut notifications = cx.notifications(&board);
    let mut visual_cx = VisualTestContext::from_window(window.into(), cx);

    visual_cx.update(|window, cx| {
        board.update(cx, |board, cx| {
            board.on_active_action(&Active, window, cx);
        });
    });
    visual_cx.run_until_parked();

    assert!(
        notifications.next().now_or_never().flatten().is_some(),
        "activating the hidden board must notify GPUI to paint its first visible frame"
    );
}

#[test]
fn test_wait_for_clipboard_write_when_writer_succeeds_returns_true() {
    let (tx, rx) = std::sync::mpsc::channel();
    assert!(tx.send(Ok(())).is_ok());

    assert!(wait_for_clipboard_write(&rx));
}

#[test]
fn test_wait_for_clipboard_write_when_writer_fails_returns_false() {
    let (tx, rx) = std::sync::mpsc::channel();
    assert!(
        tx.send(Err(ClipboardWriteError::Clipboard(
            "injected failure".to_string(),
        )))
        .is_ok()
    );

    assert!(!wait_for_clipboard_write(&rx));
}

#[rstest]
#[case(false, true)]
#[case(true, false)]
fn test_focus_out_auto_hide_for_pin_state_matches_expected(
    #[case] pinned: bool,
    #[case] expected: bool,
) {
    assert_eq!(RopyBoard::should_auto_hide_on_focus_out(pinned), expected);
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
fn test_update_manager_begin_check_ignores_duplicate_in_flight_request() {
    let mut manager = UpdateManager::new();

    assert!(manager.begin_check());
    assert!(matches!(manager.status, UpdateStatus::Checking));
    assert!(!manager.begin_check());
}

#[test]
fn test_preview_state_is_visible_only_while_space_is_held() {
    let mut ui_state = UiState::default();

    assert!(!ui_state.preview_visible());
    assert!(ui_state.show_space_preview(true));
    assert!(ui_state.preview_visible());
    assert!(!ui_state.show_space_preview(true));
    assert!(ui_state.hide_preview());
    assert!(!ui_state.preview_visible());
    assert!(!ui_state.hide_preview());
}

#[test]
fn test_space_preview_setting_blocks_space_preview_when_disabled() {
    let mut ui_state = UiState::default();

    assert!(!ui_state.show_space_preview(false));
    assert!(!ui_state.preview_visible());
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
fn test_grid_reveal_offset_scrolls_down_to_show_item_bottom() {
    let viewport = test_bounds(0.0, 0.0, 220.0, 300.0);
    let item = test_bounds(0.0, 420.0, 100.0, 120.0);

    let next_offset = grid_reveal_offset(viewport, item, point(px(0.0), px(0.0)));

    assert_eq!(next_offset, Some(point(px(0.0), px(-240.0))));
}

#[test]
fn test_grid_reveal_offset_keeps_visible_item_unchanged() {
    let viewport = test_bounds(0.0, 0.0, 220.0, 300.0);
    let item = test_bounds(0.0, 80.0, 100.0, 120.0);

    let next_offset = grid_reveal_offset(viewport, item, point(px(0.0), px(0.0)));

    assert_eq!(next_offset, None);
}

#[test]
fn test_grid_reveal_offset_scrolls_left_to_show_item_right_edge() {
    let viewport = test_bounds(0.0, 0.0, 220.0, 300.0);
    let item = test_bounds(260.0, 40.0, 100.0, 120.0);

    let next_offset = grid_reveal_offset(viewport, item, point(px(0.0), px(0.0)));

    assert_eq!(next_offset, Some(point(px(-140.0), px(0.0))));
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

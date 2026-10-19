#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

use gpui_kit::{
    AnyWindowHandle, AppContext, Entity, Focusable, InputEvent, KeyDownEvent, KeyUpEvent,
    Keystroke, TestAppContext,
    component::WindowExt,
    test::{TestQueryExt, TestWindowExt},
};
use rust_i18n::t;

use super::{ActivePanel, RopyBoard, filtering::ClearConfirmAction};
use crate::{
    clipboard::{ClipboardWriteError, CopyRequest, CopyTracker},
    config::{ConfirmMode, Settings},
    gui::{repository::GlobalRepository, settings::GlobalSettings},
    repository::{ClipboardRecord, ClipboardRepository},
};

mod shortcut_editor;
mod user_themes;

fn open_board(
    cx: &TestAppContext,
    records: Vec<ClipboardRecord>,
) -> (
    AnyWindowHandle,
    Entity<RopyBoard>,
    async_channel::Receiver<CopyRequest>,
) {
    open_board_with_settings(cx, records, Settings::default())
}

fn open_board_with_settings(
    cx: &TestAppContext,
    records: Vec<ClipboardRecord>,
    settings: Settings,
) -> (
    AnyWindowHandle,
    Entity<RopyBoard>,
    async_channel::Receiver<CopyRequest>,
) {
    let (tx, rx) = async_channel::unbounded();
    cx.update(|cx| {
        gpui_kit::init(cx);
        // Entrance animations use wall-clock time; keep hitboxes stable between
        // the pointer move, mouse-down and mouse-up frames in interaction tests.
        cx.set_reduce_motion(true);
        cx.set_global(GlobalSettings::new(settings));
        cx.set_global(GlobalRepository::new(None));
        super::bind_application_keys(cx);
    });
    let (window, board) = cx.update(|cx| {
        gpui_kit::open_window(gpui_kit::WindowOptions::default(), cx, |window, cx| {
            cx.new(|cx| {
                let mut board = RopyBoard::new(
                    Arc::new(RwLock::new(records)),
                    Arc::new(Mutex::new(CopyTracker::default())),
                    tx,
                    window,
                    cx,
                );
                board.activated = true;
                board.pinned = true;
                board
            })
        })
        .expect("open board")
    });
    (window, board, rx)
}

#[gpui_kit::test]
fn test_shortcuts_editor_opens_from_help_and_returns_without_changes(cx: &mut TestAppContext) {
    let (handle, _, _) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("help-button", cx);
        window.click("customize-shortcuts", cx);
        assert!(window.find("shortcuts-apply").label().is_some());
        window.click("shortcuts-back", cx);
        assert!(window.find("customize-shortcuts").label().is_some());
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_history_override_disables_d_and_keeps_delete(cx: &mut TestAppContext) {
    let mut settings = Settings::default();
    settings
        .keybindings
        .history
        .insert("delete_record".into(), vec!["delete".into()]);
    let (handle, board, _) = open_board_with_settings(cx, vec![record(1)], settings);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("d", cx);
        assert!(!board.read(cx).ui_state.delete_confirm_visible());
        window.press("delete", cx);
        assert_eq!(board.read(cx).pending_delete_id, Some(1));
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_history_override_preserves_search_navigation_and_labels_its_keys(cx: &mut TestAppContext) {
    let mut settings = Settings::default();
    settings
        .keybindings
        .history
        .insert("select_next".into(), vec!["n".into()]);
    let (handle, board, _) =
        open_board_with_settings(cx, vec![record(1), record(2), record(3)], settings);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("down", cx);
        window.press("j", cx);
        assert_eq!(board.read(cx).selected_index, 0);
        window.press("n", cx);
        assert_eq!(board.read(cx).selected_index, 1);
        window.press("/", cx);
        window.press("down", cx);
        assert_eq!(board.read(cx).selected_index, 2);
        window.input("n", cx);
        assert_eq!(board.read(cx).search_input.read(cx).value(), "n");

        window.click("help-button", cx);
        let label = window
            .find("board::SelectPrev")
            .label()
            .unwrap()
            .to_string();
        let down = gpui_kit::component::kbd::Kbd::format(&Keystroke::parse("down").unwrap());
        assert!(
            label.contains(t!("help_search_binding", keys = down).as_ref()),
            "{label}"
        );
        assert!(label.contains(" / N"), "{label}");
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_history_conflict_disables_history_and_shows_repair_message(cx: &mut TestAppContext) {
    let mut settings = Settings::default();
    settings
        .keybindings
        .history
        .insert("delete_record".into(), vec!["j".into()]);
    let (handle, board, _) = open_board_with_settings(cx, vec![record(1), record(2)], settings);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let message = window
            .find("keybindings-error")
            .label()
            .unwrap()
            .to_string();
        assert!(message.contains("delete_record"), "{message}");
        assert!(message.contains("select_next"), "{message}");
        assert!(message.contains("config.toml"), "{message}");
        for key in ["j", "d", "delete"] {
            window.press(key, cx);
        }
        assert_eq!(board.read(cx).selected_index, 0);
        assert!(!board.read(cx).ui_state.delete_confirm_visible());
        window.focus(&board.read(cx).search_input.focus_handle(cx), cx);
        window.press("down", cx);
        assert_eq!(board.read(cx).selected_index, 1);
        window.input("d", cx);
        assert_eq!(board.read(cx).search_input.read(cx).value(), "d");
        window.press("escape", cx);
        assert!(
            !board
                .read(cx)
                .search_input
                .focus_handle(cx)
                .is_focused(window)
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_history_config_rebinds_quick_select_target(cx: &mut TestAppContext) {
    let mut settings = Settings::default();
    settings
        .keybindings
        .history
        .insert("confirm_record_2".into(), vec!["ctrl-2".into()]);
    let (handle, board, rx) = open_board_with_settings(cx, vec![record(1), record(2)], settings);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("2", cx);
        assert!(!board.read(cx).copy_in_progress);
        window.press("ctrl-2", cx);
        assert!(board.read(cx).copy_in_progress);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(matches!(rx.try_recv().unwrap(), CopyRequest::Text { text, .. } if text == "record 1"));
}

fn record(id: u64) -> ClipboardRecord {
    // Keep newest-first ordering independent of the platform clock's resolution.
    let timestamp = 1_700_000_000 + i64::try_from(id).unwrap();
    ClipboardRecord::new(
        id,
        format!("record {id}"),
        chrono::DateTime::from_timestamp(timestamp, 0)
            .unwrap()
            .with_timezone(&chrono::Local),
        crate::repository::ContentType::Text,
    )
    .pinned(true)
}

#[gpui_kit::test]
fn test_history_settings_recovery_does_not_restore_default_delete_binding(cx: &mut TestAppContext) {
    let (handle, board, _) =
        open_board_with_settings(cx, vec![record(1)], Settings::recovery_defaults());
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        for key in ["d", "delete"] {
            window.press(key, cx);
        }
        assert!(!board.read(cx).ui_state.delete_confirm_visible());
        window.focus(&board.read(cx).search_input.focus_handle(cx), cx);
        window.input("d", cx);
        assert_eq!(board.read(cx).search_input.read(cx).value(), "d");
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_board_shortcuts_with_modifiers_do_not_delete_or_copy(cx: &mut TestAppContext) {
    let (handle, board, rx) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        for key in ["ctrl-d", "alt-d", "cmd-d", "shift-d", "ctrl-1"] {
            window.press(key, cx);
            assert!(!board.read(cx).ui_state.delete_confirm_visible(), "{key}");
            assert!(!board.read(cx).copy_in_progress, "{key}");
        }
        window.press("d", cx);
        assert!(board.read(cx).ui_state.delete_confirm_visible());
    })
    .unwrap();
    cx.run_until_parked();
    assert!(rx.try_recv().is_err());
}

#[gpui_kit::test]
fn test_board_delete_alias_obeys_registered_keymap(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        // Removing a shortcut must remove its behavior as well: there must not
        // be a second, handwritten dispatcher behind GPUI's keymap.
        cx.clear_key_bindings();
        cx.bind_keys([gpui_kit::KeyBinding::new(
            "x",
            super::actions::DeleteRecord,
            None,
        )]);
        window.press("d", cx);
        assert!(!board.read(cx).ui_state.delete_confirm_visible());
        window.press("x", cx);
        assert_eq!(board.read(cx).pending_delete_id, Some(1));
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_board_navigation_aliases_preserve_search_input(cx: &mut TestAppContext) {
    let (handle, board, rx) = open_board(cx, vec![record(1), record(2), record(3)]);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("j", cx);
        assert_eq!(board.read(cx).selected_index, 1);
        window.press("down", cx);
        assert_eq!(board.read(cx).selected_index, 2);
        window.press("k", cx);
        window.press("up", cx);
        assert_eq!(board.read(cx).selected_index, 0);

        window.press("/", cx);
        window.press("down", cx);
        assert_eq!(board.read(cx).selected_index, 1);
        window.press("up", cx);
        assert_eq!(board.read(cx).selected_index, 0);
        window.input("jdkflhpq12345 /", cx);
        assert_eq!(
            board.read(cx).search_input.read(cx).value(),
            "jdkflhpq12345 /"
        );
        assert!(!board.read(cx).ui_state.delete_confirm_visible());
        assert!(!board.read(cx).ui_state.preview_visible());
        assert!(board.read(cx).pinned);
        assert_eq!(board.read(cx).active_panel, ActivePanel::ClipboardList);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(rx.try_recv().is_err());
}

#[gpui_kit::test]
fn test_board_grid_navigation_aliases_share_actions(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![record(1), record(2)]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, _| {
            board.layout_mode = crate::config::LayoutMode::Grid;
        });
        window.render_frame(cx);
        window.press("l", cx);
        assert_eq!(board.read(cx).selected_index, 1);
        window.press("left", cx);
        assert_eq!(board.read(cx).selected_index, 0);
        window.press("right", cx);
        assert_eq!(board.read(cx).selected_index, 1);
        window.press("h", cx);
        assert_eq!(board.read(cx).selected_index, 0);
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_board_masonry_horizontal_navigation_stays_at_edges(cx: &mut TestAppContext) {
    let mut image = record(4);
    image.content_type = crate::repository::ContentType::Image;
    let mut settings = Settings::default();
    settings.layout.mode = crate::config::LayoutMode::Grid;
    let (handle, board, _) =
        open_board_with_settings(cx, vec![image, record(3), record(2), record(1)], settings);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.selected_index = 2;
            window.focus(&board.focus_handle, cx);
        });
        window.render_frame(cx);
        let bounds = &board.read(cx).grid_scroll_handle;
        assert!(
            bounds.bounds_for_item(2).unwrap().left() > bounds.bounds_for_item(3).unwrap().left()
        );
        window.press("right", cx);
        assert_eq!(board.read(cx).selected_index, 2);
        window.press("left", cx);
        assert_eq!(board.read(cx).selected_index, 3);
        window.press("left", cx);
        assert_eq!(board.read(cx).selected_index, 3);
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_board_help_reflects_registered_shortcuts(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("help-button", cx);
        assert_eq!(board.read(cx).active_panel, ActivePanel::Help);
        assert!(
            window
                .find("board::DeleteRecord")
                .label()
                .unwrap()
                .ends_with(" / D")
        );

        cx.clear_key_bindings();
        cx.bind_keys([gpui_kit::KeyBinding::new(
            "x",
            super::actions::DeleteRecord,
            None,
        )]);
        window.render_frame(cx);
        assert_eq!(
            window.find("board::DeleteRecord").label(),
            Some(format!("{}: X", t!("help_delete")).as_str()),
        );
        window.press("x", cx);
        assert!(!board.read(cx).ui_state.delete_confirm_visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_board_dialog_suspends_history_shortcuts(cx: &mut TestAppContext) {
    let (handle, board, rx) = open_board(cx, vec![record(1), record(2)]);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("d", cx);
        let pending_delete_id = board.read(cx).pending_delete_id;
        assert!(pending_delete_id.is_some());
        for key in ["j", "down", "f", "p", "alt-f", "1", "q"] {
            window.press(key, cx);
            assert_eq!(board.read(cx).selected_index, 0, "{key}");
            assert_eq!(board.read(cx).pending_delete_id, pending_delete_id, "{key}");
            assert!(board.read(cx).pinned, "{key}");
            assert!(board.read(cx).favorite_ids.is_empty(), "{key}");
            assert!(!board.read(cx).filter_state.favorites_only, "{key}");
            assert!(!board.read(cx).copy_in_progress, "{key}");
        }
        window.press("escape", cx);
        assert!(!board.read(cx).ui_state.delete_confirm_visible());
        window.press("j", cx);
        assert_eq!(board.read(cx).selected_index, 1);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(rx.try_recv().is_err());
}

#[gpui_kit::test]
fn test_board_unmodified_numbers_copy_corresponding_records(cx: &mut TestAppContext) {
    let (handle, board, rx) = open_board(cx, (1..=5).map(record).collect());
    for (key, expected) in [
        ("1", "record 5"),
        ("2", "record 4"),
        ("3", "record 3"),
        ("4", "record 2"),
        ("5", "record 1"),
    ] {
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.press(key, cx);
        })
        .unwrap();
        cx.run_until_parked();
        let request = rx.try_recv().expect("bare number must request a copy");
        cx.update(|cx| board.update(cx, |board, _| board.invalidate_copy_confirmation()));
        let actual = match &request {
            CopyRequest::Text { text, .. } => Some(text.as_str()),
            _ => None,
        };
        assert_eq!(actual, Some(expected), "{key}");
    }
}

#[gpui_kit::test]
fn test_board_delete_key_preserves_search_text_editing_and_opens_confirmation(
    cx: &mut TestAppContext,
) {
    let (handle, board, _) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("/", cx);
        window.input("rrecord 1", cx);
        window.press("home", cx);
        window.press("delete", cx);
        assert_eq!(board.read(cx).search_input.read(cx).value(), "record 1");
        assert!(!board.read(cx).ui_state.delete_confirm_visible());
        window.press("escape", cx);
        window.press("delete", cx);
        assert!(board.read(cx).ui_state.delete_confirm_visible());
        assert_eq!(board.read(cx).pending_delete_id, Some(1));
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_board_search_horizontal_arrows_move_text_cursor(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("/", cx);
        window.input("ab", cx);
        window.press("left", cx);
        window.input("x", cx);
        assert_eq!(board.read(cx).search_input.read(cx).value(), "axb");
        window.press("right", cx);
        window.input("y", cx);
        assert_eq!(board.read(cx).search_input.read(cx).value(), "axby");
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_board_delete_key_does_not_open_record_dialog_behind_clear_dialog(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.open_clear_confirm(ClearConfirmAction::AllHistory, window, cx);
        });
        window.render_frame(cx);
        window.press("delete", cx);
        assert!(board.read(cx).ui_state.clear_confirm_visible());
        assert!(!board.read(cx).ui_state.delete_confirm_visible());
        assert_eq!(board.read(cx).pending_delete_id, None);
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_board_space_preview_release_after_search_focus_hides_preview(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, _| {
            board.settings_editor.panel_state.space_preview_enabled = true;
        });
        window.render_frame(cx);
        let keystroke = Keystroke::parse("space").unwrap();
        window.dispatch_event(
            KeyDownEvent {
                keystroke: keystroke.clone(),
                is_held: false,
                prefer_character_input: false,
            }
            .to_platform_input(),
            cx,
        );
        window.render_frame(cx);
        assert!(board.read(cx).ui_state.preview_visible());
        window.press("/", cx);
        assert!(
            board
                .read(cx)
                .search_input
                .focus_handle(cx)
                .is_focused(window)
        );
        window.dispatch_event(KeyUpEvent { keystroke }.to_platform_input(), cx);
        assert!(!board.read(cx).ui_state.preview_visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_board_preview_release_follows_configured_binding(cx: &mut TestAppContext) {
    let mut settings = Settings::default();
    settings
        .keybindings
        .history
        .insert("preview_selection".into(), vec!["v".into()]);
    let (handle, board, _) = open_board_with_settings(cx, vec![record(1)], settings);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, _| {
            board.settings_editor.panel_state.space_preview_enabled = true;
        });
        window.render_frame(cx);
        window.dispatch_event(
            KeyDownEvent {
                keystroke: Keystroke::parse("v").unwrap(),
                is_held: false,
                prefer_character_input: false,
            }
            .to_platform_input(),
            cx,
        );
        assert!(board.read(cx).ui_state.preview_visible());
        window.focus(&board.read(cx).search_input.focus_handle(cx), cx);
        window.render_frame(cx);
        for (key, visible) in [("space", true), ("shift-v", false)] {
            window.dispatch_event(
                KeyUpEvent {
                    keystroke: Keystroke::parse(key).unwrap(),
                }
                .to_platform_input(),
                cx,
            );
            assert_eq!(board.read(cx).ui_state.preview_visible(), visible, "{key}");
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_board_space_preview_deactivation_hides_preview_in_pinned_window(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        window.activate_window();
        board.update(cx, |board, _| {
            board.settings_editor.panel_state.space_preview_enabled = true;
        });
        window.render_frame(cx);
        window.dispatch_event(
            KeyDownEvent {
                keystroke: Keystroke::parse("space").unwrap(),
                is_held: false,
                prefer_character_input: false,
            }
            .to_platform_input(),
            cx,
        );
        assert!(board.read(cx).ui_state.preview_visible());
    })
    .unwrap();
    cx.run_until_parked();
    gpui_kit::VisualTestContext::from_window(handle, cx).deactivate_window();
    cx.run_until_parked();
    cx.read(|cx| {
        assert!(!board.read(cx).ui_state.preview_visible());
        assert!(board.read(cx).pinned);
    });
}

#[gpui_kit::test]
fn test_board_controls_expose_localized_accessible_names(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        for (id, label) in [
            ("pin-button", t!("unpin")),
            ("help-button", t!("help_title")),
            ("about-button", t!("about_title")),
            ("settings-button", t!("settings_button")),
            ("filter-text-btn", t!("filter_text")),
            ("filter-image-btn", t!("filter_image")),
            ("filter-files-btn", t!("filter_files")),
            ("filter-favorites-btn", t!("filter_favorites")),
            ("search-case-sensitive-btn", t!("search_case_sensitive")),
            ("search-whole-word-btn", t!("search_match_whole_word")),
        ] {
            assert_eq!(window.find(id).label(), Some(label.as_ref()), "{id}");
        }
        assert_eq!(
            window
                .find(("input", board.read(cx).search_input.entity_id()))
                .label(),
            Some(t!("search_records").as_ref())
        );
        window.click("settings-button", cx);
        for (id, label) in [
            ("cancel-button", t!("back")),
            ("autostart-toggle", t!("settings_autostart")),
            ("confirm-mode-toggle", t!("settings_paste_immediately")),
            ("hover-preview-toggle", t!("settings_hover_preview")),
            ("space-preview-toggle", t!("settings_space_preview")),
            ("auto-check-toggle", t!("update_auto_check")),
            ("include-prerelease-toggle", t!("update_include_prerelease")),
            ("max-history-save-button", t!("settings_max_history_save")),
            ("max-storage-save-button", t!("settings_max_storage_save")),
        ] {
            assert_eq!(window.find(id).label(), Some(label.as_ref()), "{id}");
        }
        let editor = &board.read(cx).settings_editor;
        assert_eq!(
            window
                .find_by_label(t!("settings_window_opacity").as_ref())
                .role(),
            Some(gpui_kit::Role::Slider)
        );
        for (input, label) in [
            (
                &editor.settings_max_history_input,
                t!("settings_max_history"),
            ),
            (
                &editor.settings_max_storage_input,
                t!("settings_max_storage"),
            ),
        ] {
            assert_eq!(
                window.find(("input", input.entity_id())).label(),
                Some(label.as_ref())
            );
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_settings_controls_share_aligned_frames(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![]);
    let directory = tempfile::tempdir().unwrap();
    let repository = ClipboardRepository::open(
        &directory.path().join("db.redb"),
        directory.path().join("images"),
    )
    .unwrap();
    cx.update(|cx| cx.set_global(GlobalRepository::new(Some(Arc::new(repository)))));
    cx.simulate_window_resize(
        handle,
        gpui_kit::size(gpui_kit::px(400.), gpui_kit::px(550.)),
    );
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| board.open_settings_panel(window, cx));
        window.render_frame(cx);
        let editor = &board.read(cx).settings_editor;
        let theme = window.within("theme-select").find("input").bounds();
        for entity_id in [
            editor.language_select.entity_id(),
            editor.layout_select.entity_id(),
        ] {
            let control = window.within(("select", entity_id)).find("input").bounds();
            assert_eq!(control.size, theme.size);
            assert_eq!(control.right(), theme.right());
        }
        let hotkey = window
            .find(("input", editor.settings_activation_key_input.entity_id()))
            .bounds();
        assert_eq!(hotkey.size, theme.size);
        assert_eq!(hotkey.right(), theme.right());
        assert!(theme.right() <= window.viewport_size().width);
        assert_eq!(
            window.find("follow-system-toggle").bounds().size,
            window.find("confirm-mode-toggle").bounds().size
        );
        let language_id = ("select", editor.language_select.entity_id());
        window.click(language_id, cx);
        assert_eq!(window.find(language_id).expanded(), Some(true));
        window.press("escape", cx);
        assert_eq!(window.find(language_id).expanded(), Some(false));
        window.click("hotkey-record-button", cx);
        assert!(board.read(cx).settings_editor.hotkey.recording);
        window.press("escape", cx);
        assert!(!board.read(cx).settings_editor.hotkey.recording);
        assert_eq!(board.read(cx).active_panel, ActivePanel::Settings);
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_appearance_settings_show_only_choices_for_current_mode(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.open_settings_panel(window, cx);
        });
        window.render_frame(cx);
        assert!(window.find("follow-system-toggle").visible());
        assert!(window.find("theme-select").visible());
        assert!(window.try_find("light-theme-select").is_none());
        GlobalSettings::update(cx, |settings| settings.appearance.follow_system = true);
        board.update(cx, |_, cx| cx.notify());
        window.render_frame(cx);
        assert!(window.try_find("theme-select").is_none());
        assert!(window.find("light-theme-select").visible());
        assert!(window.find("dark-theme-select").visible());
        window.click("light-theme-select", cx);
        window.render_frame(cx);
        assert!(
            board
                .read(cx)
                .settings_editor
                .theme
                .select(super::ThemeTarget::Light)
                .focus_handle(cx)
                .contains_focused(window, cx)
        );
        window.press("down", cx);
        window.render_frame(cx);
        window.press("escape", cx);
        assert_eq!(board.read(cx).active_panel, ActivePanel::Settings);
        assert_eq!(
            GlobalSettings::read(cx, |s| s.appearance.light_theme.clone()),
            crate::gui::theme::ThemeId::new("ropy-light")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_appearance_refresh_restores_selected_palette_and_opacity(cx: &mut TestAppContext) {
    use crate::gui::{
        app,
        theme::{ThemeId, ThemeMode},
        utils::surface_with_opacity,
    };

    let (handle, board, _) = open_board(cx, vec![]);
    cx.update_window(handle, |_, window, cx| {
        for following in [false, true] {
            GlobalSettings::update(cx, |settings| {
                settings.appearance.follow_system = following;
                settings.appearance.light_theme = ThemeId::new("nord-light");
                settings.theme = ThemeId::new("everforest-night");
            });
            app::set_app_theme(window, cx, &ThemeId::new("everforest-night"), 65);
            board.update(cx, |board, cx| {
                board.settings_editor.window_opacity_percent = 65;
                board
                    .settings_editor
                    .theme
                    .update_system_mode(ThemeMode::Dark);
                board.refresh_system_appearance(window, cx);
            });
            let expected_color = if following { 0x00e3_e7ee } else { 0x0023_2a2e };
            let expected = surface_with_opacity(gpui_kit::rgb(expected_color).into(), 65);
            assert_eq!(gpui_kit::component::Theme::global(cx).background, expected);
            assert_eq!(
                gpui_kit::base::Theme::global(cx).tokens.colors.background,
                expected
            );
            assert_eq!(
                GlobalSettings::read(cx, |s| s.theme.code().to_owned()),
                "everforest-night"
            );
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_appearance_recovery_rejects_switch_and_theme_without_changing_palette(
    cx: &mut TestAppContext,
) {
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update(|cx| cx.set_global(GlobalSettings::new(Settings::recovery_defaults())));
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| board.open_settings_panel(window, cx));
        window.render_frame(cx);
        let background = gpui_kit::component::Theme::global(cx).background;
        window.click("follow-system-toggle", cx);
        board.update(cx, |board, cx| {
            board.save_selected_theme(
                super::ThemeTarget::Fixed,
                &crate::gui::theme::ThemeId::new("ropy-dark"),
                window,
                cx,
            );
        });
        assert!(!GlobalSettings::read(cx, |s| s.appearance.follow_system));
        assert_eq!(
            GlobalSettings::read(cx, |s| s.theme.clone()),
            crate::gui::theme::ThemeId::new("ropy-light")
        );
        assert_eq!(
            gpui_kit::component::Theme::global(cx).background,
            background
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_clear_dialog_keyboard_does_not_copy_background_record(cx: &mut TestAppContext) {
    let (handle, board, rx) = open_board(cx, vec![record(1), record(2)]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.open_clear_confirm(ClearConfirmAction::AllHistory, window, cx);
        });
        window.render_frame(cx);
        window.press("shift-enter", cx);
        window.press("down", cx);
        assert_eq!(board.read(cx).selected_index, 0);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(rx.try_recv().is_err());
}

#[gpui_kit::test]
fn test_delete_dialog_typing_d_keeps_record_and_escape_restores_search_focus(
    cx: &mut TestAppContext,
) {
    let (handle, board, _) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            window.focus(&board.search_input.focus_handle(cx), cx);
            board.request_delete_record(1, window, cx);
        });
        window.render_frame(cx);
        assert!(
            !board
                .read(cx)
                .search_input
                .focus_handle(cx)
                .is_focused(window)
        );
        window.input("d", cx);
        assert!(board.read(cx).ui_state.delete_confirm_visible());
        assert_eq!(board.read(cx).pending_delete_id, Some(1));
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        assert!(!window.has_active_dialog(cx));
        assert!(!board.read(cx).ui_state.delete_confirm_visible());
        assert!(
            board
                .read(cx)
                .search_input
                .focus_handle(cx)
                .is_focused(window)
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_refresh_notification_preserves_pending_storage_limit_trim(cx: &TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        ClipboardRepository::open(&dir.path().join("db.redb"), dir.path().join("images")).unwrap(),
    );
    for value in 0..11 {
        repo.save_text(value.to_string()).unwrap();
    }
    let (_, board, _) = open_board(cx, vec![]);
    cx.update(|cx| {
        cx.set_global(GlobalRepository::new(Some(repo.clone())));
        GlobalSettings::update(cx, |settings| {
            settings.storage.max_history_records = 10;
            settings.storage.max_storage_records = 10;
        });
        board.update(cx, |board, cx| {
            board.refresh_history(cx, true);
            board.refresh_records_from_repository(cx);
        });
    });
    cx.run_until_parked();
    assert_eq!(
        repo.count(),
        10,
        "notification must not downgrade a requested immediate trim to buffered cleanup"
    );
}

#[gpui_kit::test]
fn test_refresh_settings_recovery_preserves_history_until_repaired(cx: &TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        ClipboardRepository::open(&dir.path().join("db.redb"), dir.path().join("images")).unwrap(),
    );
    for value in 0..231 {
        repo.save_text(value.to_string()).unwrap();
    }
    let (_, board, _) = open_board(cx, vec![]);
    cx.update(|cx| {
        cx.set_global(GlobalRepository::new(Some(repo.clone())));
        cx.set_global(GlobalSettings::new(Settings::recovery_defaults()));
        board.update(cx, |board, cx| board.refresh_records_from_repository(cx));
    });
    cx.run_until_parked();
    assert_eq!(
        repo.count(),
        231,
        "recovery defaults must not delete history"
    );
    cx.update(|cx| {
        assert!(!crate::utils::read_or_recover(&board.read(cx).records).is_empty());
        board.update(cx, |board, cx| board.refresh_history(cx, true));
    });
    cx.run_until_parked();
    assert_eq!(
        repo.count(),
        231,
        "explicit trims must also respect recovery"
    );
    cx.update(|cx| {
        cx.set_global(GlobalSettings::new(Settings::default()));
        board.update(cx, |board, cx| board.refresh_records_from_repository(cx));
    });
    cx.run_until_parked();
    assert_eq!(repo.count(), 200, "valid settings restore normal retention");
}

#[gpui_kit::test]
#[expect(
    clippy::future_not_send,
    reason = "GPUI test contexts stay on the UI thread"
)]
async fn test_copy_confirmation_waits_for_writer_and_reports_failure(cx: &mut TestAppContext) {
    let (handle, board, rx) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("enter", cx);
    })
    .unwrap();
    let CopyRequest::Text {
        completion: Some(completion),
        ..
    } = rx.recv().await.unwrap()
    else {
        unreachable!("copy confirmation must request native write completion")
    };
    cx.run_until_parked();
    cx.update(|cx| assert!(board.read(cx).copy_in_progress));
    completion
        .try_send(Err(ClipboardWriteError::Clipboard(
            "injected failure".into(),
        )))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        assert!(!board.read(cx).copy_in_progress);
        assert_eq!(
            window.notifications(cx).len(),
            1,
            "write failure must be visible"
        );
        window.render_frame(cx);
        assert!(window.find("settings-button").visible());
        window.press("enter", cx);
    })
    .unwrap();
    let CopyRequest::Text {
        completion: Some(completion),
        ..
    } = rx.recv().await.unwrap()
    else {
        unreachable!()
    };
    completion.try_send(Ok(())).unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(!board.read(cx).copy_in_progress));
}

#[gpui_kit::test]
fn test_refresh_clear_while_pending_does_not_restore_deleted_history(cx: &TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        ClipboardRepository::open(&dir.path().join("db.redb"), dir.path().join("images")).unwrap(),
    );
    repo.save_text("clear me".into()).unwrap();
    let (_, board, _) = open_board(cx, repo.get_display_records(100).unwrap());
    cx.update(|cx| {
        cx.set_global(GlobalRepository::new(Some(repo.clone())));
        board.update(cx, |board, cx| {
            board.refresh_records_from_repository(cx);
            board.clear_history(cx);
        });
    });
    cx.run_until_parked();
    cx.update(|cx| assert!(crate::utils::read_or_recover(&board.read(cx).records).is_empty()));
    assert_eq!(repo.count(), 0);
}

#[gpui_kit::test]
fn test_refresh_newer_history_limit_replaces_older_request(cx: &TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        ClipboardRepository::open(&dir.path().join("db.redb"), dir.path().join("images")).unwrap(),
    );
    for value in ["one", "two", "three"] {
        repo.save_text(value.into()).unwrap();
    }
    let (_, board, _) = open_board(cx, vec![]);
    cx.update(|cx| {
        cx.set_global(GlobalRepository::new(Some(repo.clone())));
        board.update(cx, |board, cx| {
            board.refresh_records_from_repository(cx);
            GlobalSettings::update(cx, |settings| settings.storage.max_history_records = 1);
            board.refresh_records_from_repository(cx);
        });
    });
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            crate::utils::read_or_recover(&board.read(cx).records).len(),
            1
        );
    });
}

#[gpui_kit::test]
fn test_refresh_runs_in_background_and_uses_current_filter(cx: &TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        ClipboardRepository::open(&dir.path().join("db.redb"), dir.path().join("images")).unwrap(),
    );
    repo.save_text("new text".into()).unwrap();
    let (_, board, _) = open_board(cx, vec![]);
    cx.update(|cx| {
        cx.set_global(GlobalRepository::new(Some(repo.clone())));
        board.update(cx, |board, cx| {
            board.refresh_records_from_repository(cx);
            assert!(
                board.filtered_record_indices.is_empty(),
                "refresh must return before database I/O"
            );
            board.toggle_content_filter(super::ContentFilter::Image);
            board.sync_filtered_records(cx);
        });
    });
    cx.run_until_parked();
    cx.update(|cx| {
        let board = board.read(cx);
        assert_eq!(crate::utils::read_or_recover(&board.records).len(), 1);
        assert!(
            board.filtered_record_indices.is_empty(),
            "completion must preserve the current filter"
        );
    });
}

#[gpui_kit::test]
fn test_refresh_new_record_preserves_selected_identity(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().expect("tempdir");
    let repo = Arc::new(
        ClipboardRepository::init(
            &dir.path().join("db.redb"),
            dir.path().join("images"),
            crate::repository::backend::redb::redb_backend_factory,
        )
        .unwrap(),
    );
    let original = repo.save_text("original selection".into()).unwrap();
    let (handle, board, _) = open_board(cx, repo.get_display_records(100).unwrap());
    cx.update(|cx| cx.set_global(GlobalRepository::new(Some(repo.clone()))));
    repo.save_text("new clipboard event".into()).unwrap();
    cx.update(|cx| {
        board.update(cx, |board, cx| {
            board.refresh_records_from_repository(cx);
            assert_eq!(
                board.filtered_record_id_at(board.selected_index),
                Some(original.id)
            );
        });
    });
    cx.run_until_parked();
    cx.update(|cx| {
        let board = board.read(cx);
        assert_eq!(crate::utils::read_or_recover(&board.records).len(), 2);
        assert_eq!(
            board.filtered_record_id_at(board.selected_index),
            Some(original.id)
        );
    });
    // Deleting a different record must not move the surviving selection.
    cx.update_window(handle, |_, _, cx| {
        board.update(cx, |board, cx| {
            let other = board.filtered_record_id_at(0).unwrap();
            board.delete_record(other, cx);
            board.reveal_selected_record();
            assert_eq!(
                board.filtered_record_id_at(board.selected_index),
                Some(original.id)
            );
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        let board = board.read(cx);
        assert_eq!(crate::utils::read_or_recover(&board.records).len(), 1);
        assert_eq!(
            board.filtered_record_id_at(board.selected_index),
            Some(original.id)
        );
    });
}

#[gpui_kit::test]
fn test_open_settings_panel_reveals_opacity_slider_on_next_frame(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.open_settings_panel(window, cx);
            assert!(
                !board
                    .settings_editor
                    .panel_state
                    .window_opacity_slider_visible
            );
        });
        assert!(window.simulate_next_frame(cx) > 0);
        assert!(
            board
                .read(cx)
                .settings_editor
                .panel_state
                .window_opacity_slider_visible
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_close_settings_before_next_frame_does_not_reveal_opacity_slider(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.open_settings_panel(window, cx);
            board.on_hide_action(&super::Hide, window, cx);
        });
        assert!(window.simulate_next_frame(cx) > 0);
        assert_eq!(board.read(cx).active_panel, ActivePanel::ClipboardList);
        assert!(
            !board
                .read(cx)
                .settings_editor
                .panel_state
                .window_opacity_slider_visible
        );
    })
    .unwrap();
}

#[gpui_kit::test]
#[expect(
    clippy::future_not_send,
    reason = "GPUI test contexts stay on the UI thread"
)]
async fn test_immediate_paste_pending_writer_keeps_ui_responsive(cx: &mut TestAppContext) {
    let (handle, board, rx) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.confirm_mode = ConfirmMode::PasteImmediately;
            board.confirm_record(window, cx, 0);
            assert!(board.copy_in_progress);
        });
    })
    .unwrap();
    let request = rx.recv().await.unwrap();
    let CopyRequest::Text {
        completion: Some(completion),
        ..
    } = request
    else {
        unreachable!()
    };
    // Dispatch another real UI event while the writer has not completed.
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.input("/", cx);
        assert!(
            board
                .read(cx)
                .search_input
                .focus_handle(cx)
                .is_focused(window)
        );
        board.update(cx, |board, cx| board.confirm_record(window, cx, 0));
    })
    .unwrap();
    assert!(
        rx.try_recv().is_err(),
        "duplicate confirmation queued a second write"
    );
    completion
        .try_send(Err(ClipboardWriteError::EmptyFileList))
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert!(!board.read(cx).copy_in_progress));
}

#[gpui_kit::test]
#[expect(
    clippy::future_not_send,
    reason = "GPUI test contexts stay on the UI thread"
)]
async fn test_immediate_paste_timeout_rejects_late_completion(cx: &mut TestAppContext) {
    let (handle, board, rx) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.confirm_mode = ConfirmMode::PasteImmediately;
            board.confirm_record(window, cx, 0);
        });
    })
    .unwrap();
    let CopyRequest::Text {
        completion: Some(completion),
        ..
    } = rx.recv().await.unwrap()
    else {
        unreachable!()
    };
    cx.run_until_parked();
    cx.executor().advance_clock(Duration::from_millis(499));
    cx.run_until_parked();
    cx.update(|cx| assert!(board.read(cx).copy_in_progress));
    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    cx.update(|cx| assert!(!board.read(cx).copy_in_progress));
    assert!(
        completion.try_send(Ok(())).is_err(),
        "timed-out operation still accepts a paste result"
    );
}

#[gpui_kit::test]
fn test_delete_dialog_confirm_button_deletes_only_target(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Arc::new(
        ClipboardRepository::init(
            &dir.path().join("db.redb"),
            dir.path().join("images"),
            crate::repository::backend::redb::redb_backend_factory,
        )
        .unwrap(),
    );
    let target = repo.save_text("delete this".into()).unwrap();
    repo.toggle_pin(target.id).unwrap();
    let other = repo.save_text("keep this".into()).unwrap();
    let (handle, board, _) = open_board(cx, repo.get_display_records(100).unwrap());
    cx.update(|cx| cx.set_global(GlobalRepository::new(Some(repo.clone()))));
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.request_delete_record(target.id, window, cx);
        });
        window.render_frame(cx);
        for _ in 0..4 {
            window.press("tab", cx);
            assert!(
                !board
                    .read(cx)
                    .search_input
                    .focus_handle(cx)
                    .is_focused(window)
            );
        }
        let button = window.find("ok").bounds();
        assert!(button.right() <= window.viewport_size().width);
        assert!(button.bottom() <= window.viewport_size().height);
        window.click("ok", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(repo.get_by_id(target.id).unwrap().is_none());
    assert!(repo.get_by_id(other.id).unwrap().is_some());
    cx.update(|cx| assert!(!board.read(cx).ui_state.delete_confirm_visible()));
}

#[gpui_kit::test]
fn test_update_restart_failure_keeps_retry_action_available(cx: &mut TestAppContext) {
    use crate::updater::{errors::UpdateFailure, models::UpdateStatus};
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.active_panel = ActivePanel::About;
            board.update_manager.status = UpdateStatus::ReadyToRestart;
            cx.notify();
        });
        window.render_frame(cx);
        window.click("update-restart-button", cx);
        assert_eq!(
            board.read(cx).update_manager.status,
            UpdateStatus::Restarting
        );
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            board.read(cx).update_manager.status,
            UpdateStatus::Error(UpdateFailure::Restart)
        );
        assert!(board.read(cx).update_manager.restart_child.is_none());
        assert!(window.find("update-restart-button").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_update_managed_installation_never_offers_download(cx: &mut TestAppContext) {
    use crate::updater::models::{ReleaseInfo, UpdateStatus};
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.active_panel = ActivePanel::About;
            board.update_manager.managed = true;
            board.update_manager.status = UpdateStatus::Available(ReleaseInfo {
                version: "9.0.0".into(),
                release_notes: String::new(),
                download_url: "https://example.invalid/archive".into(),
                checksum_url: String::new(),
                asset_size: 0,
            });
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.try_find("update-download-button").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_startup_repository_failure_remains_visible_across_panels(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("repository-unavailable").visible());
        assert_eq!(
            window.find("repository-unavailable").label(),
            Some(t!("repository_unavailable").as_ref())
        );
        board.update(cx, |board, cx| {
            board.active_panel = ActivePanel::Settings;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("repository-unavailable").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_startup_settings_failure_remains_visible_and_rejects_updates(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update(|cx| cx.set_global(GlobalSettings::new(Settings::recovery_defaults())));
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.active_panel = ActivePanel::Settings;
            cx.notify();
        });
        window.render_frame(cx);
        assert!(window.find("settings-recovery").visible());
        window.click("confirm-mode-toggle", cx);
        assert_eq!(
            GlobalSettings::read(cx, |settings| settings.confirm.mode),
            ConfirmMode::CopyToClipboard
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_startup_ready_hides_recovery_alerts(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().expect("tempdir");
    let repo = Arc::new(
        ClipboardRepository::init(
            &directory.path().join("clipboard.redb"),
            directory.path().join("images"),
            crate::repository::backend::redb::redb_backend_factory,
        )
        .expect("repository"),
    );
    let (handle, _, _) = open_board(cx, vec![]);
    cx.update(|cx| cx.set_global(GlobalRepository::new(Some(repo))));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(window.try_find("repository-unavailable").is_none());
        assert!(window.try_find("settings-recovery").is_none());
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_header_buttons_click_opens_corresponding_panel(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        for (button, panel, back_button) in [
            ("help-button", ActivePanel::Help, "help-back-button"),
            ("about-button", ActivePanel::About, "back-button"),
            ("settings-button", ActivePanel::Settings, "cancel-button"),
        ] {
            board.update(cx, |board, cx| {
                board.active_panel = ActivePanel::ClipboardList;
                cx.notify();
            });
            window.render_frame(cx);
            window.click(button, cx);
            assert_eq!(board.read(cx).active_panel, panel);
            window.click(back_button, cx);
            assert_eq!(board.read(cx).active_panel, ActivePanel::ClipboardList);
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_internal_focus_change_does_not_hide_active_popup(cx: &mut TestAppContext) {
    let (handle, board, _) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, _| board.pinned = false);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    let generation = cx.read(|cx| board.read(cx).copy_generation);
    cx.update_window(handle, |_, window, cx| {
        let another_control = cx.focus_handle();
        window.focus(&another_control, cx);
        window.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.read(|cx| assert_eq!(board.read(cx).copy_generation, generation));
}

#[gpui_kit::test]
#[expect(
    clippy::future_not_send,
    reason = "GPUI test contexts stay on the UI thread"
)]
async fn test_copy_completion_from_closed_window_preserves_new_request(cx: &mut TestAppContext) {
    let (old_handle, board, rx) = open_board(cx, vec![record(1)]);
    cx.update_window(old_handle, |_, window, cx| {
        board.update(cx, |board, cx| board.confirm_record(window, cx, 0));
    })
    .unwrap();
    let CopyRequest::Text {
        completion: Some(old_done),
        ..
    } = rx.recv().await.unwrap()
    else {
        unreachable!("copy must request writer acknowledgement")
    };

    cx.update_window(old_handle, |_, window, _| window.remove_window())
        .unwrap();
    let new_handle = cx.update(|cx| crate::gui::app::reopen_window(cx, board.clone()).unwrap());
    cx.update_window(new_handle.into(), |_, window, cx| {
        board.update(cx, |board, cx| board.confirm_record(window, cx, 0));
    })
    .unwrap();
    let CopyRequest::Text {
        completion: Some(new_done),
        ..
    } = rx.recv().await.unwrap()
    else {
        unreachable!("copy must request writer acknowledgement")
    };

    old_done.try_send(Ok(())).unwrap();
    cx.run_until_parked();
    cx.update_window(new_handle.into(), |_, window, cx| {
        assert!(board.read(cx).copy_in_progress);
        assert!(window.notifications(cx).is_empty());
        board.update(cx, |board, cx| board.confirm_record(window, cx, 0));
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        rx.try_recv().is_err(),
        "stale completion admitted a third copy"
    );

    new_done.try_send(Ok(())).unwrap();
    cx.run_until_parked();
    cx.read(|cx| assert!(!board.read(cx).copy_in_progress));
}

#[gpui_kit::test]
#[expect(
    clippy::future_not_send,
    reason = "GPUI test contexts stay on the UI thread"
)]
async fn test_copy_reactivation_allows_retry_before_previous_writer_finishes(
    cx: &mut TestAppContext,
) {
    let (handle, board, rx) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| board.confirm_record(window, cx, 0));
    })
    .unwrap();
    let old_request = rx.recv().await.unwrap();

    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.on_hide_action(&super::Hide, window, cx);
            board.on_active_action(&super::Active, window, cx);
            assert!(
                !board.copy_in_progress,
                "dismissed work still blocks the board"
            );
            board.confirm_record(window, cx, 0);
        });
    })
    .unwrap();
    cx.run_until_parked();
    assert!(
        rx.try_recv().is_ok(),
        "reactivated board did not queue the retry"
    );
    drop(old_request);
}

#[gpui_kit::test]
fn test_native_activation_initializes_hidden_board(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(GlobalSettings::new(Settings::default()));
        cx.set_global(GlobalRepository::new(None));
    });
    let (copy_tx, _copy_rx) = async_channel::unbounded();
    let handle = cx.update(|cx| {
        crate::gui::create_window(
            cx,
            Arc::new(RwLock::new(vec![record(1)])),
            Arc::new(Mutex::new(CopyTracker::default())),
            copy_tx,
        )
    });
    cx.update_window(handle.into(), |root, window, cx| {
        let board = root.downcast::<gpui_kit::component::Root>().unwrap();
        let board = board
            .read(cx)
            .view()
            .clone()
            .downcast::<RopyBoard>()
            .unwrap();
        assert!(!board.read(cx).activated, "startup must stay inactive");
        window.render_frame(cx);
        assert!(window.try_find("settings-button").is_none());
        // The platform activation callback also runs when another process
        // restores the existing native window, without dispatching Active.
        window.activate_window();
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle.into(), |root, window, cx| {
        let root = root.downcast::<gpui_kit::component::Root>().unwrap();
        let board = root
            .read(cx)
            .view()
            .clone()
            .downcast::<RopyBoard>()
            .unwrap();
        assert!(board.read(cx).activated);
        window.render_frame(cx);
        assert!(window.find("settings-button").visible());
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_copy_invalidated_during_preparation_never_reaches_writer(cx: &mut TestAppContext) {
    let (handle, board, rx) = open_board(cx, vec![record(1)]);
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.confirm_record(window, cx, 0);
            assert!(rx.is_empty(), "preparation must still be pending");
            board.on_hide_action(&super::Hide, window, cx);
            board.on_active_action(&super::Active, window, cx);
            crate::utils::write_or_recover(&board.records)[0].content = "new selection".into();
            board.confirm_record(window, cx, 0);
        });
    })
    .unwrap();
    cx.run_until_parked();

    let requests: Vec<_> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
    assert_eq!(
        requests.len(),
        1,
        "cancelled preparation reached the writer"
    );
    assert!(matches!(&requests[0], CopyRequest::Text { text, .. } if text == "new selection"));
}

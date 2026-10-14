#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

use gpui_kit::{
    AnyWindowHandle, AppContext, Entity, Focusable, KeyBinding, TestAppContext,
    component::WindowExt, test::TestWindowExt,
};
use rust_i18n::t;

use super::{ActivePanel, RopyBoard, filtering::ClearConfirmAction};
use crate::{
    clipboard::{ClipboardWriteError, CopyRequest, CopyTracker},
    config::{ConfirmMode, Settings},
    gui::{repository::GlobalRepository, settings::GlobalSettings},
    repository::{ClipboardRecord, ClipboardRepository},
};

fn open_board(
    cx: &TestAppContext,
    records: Vec<ClipboardRecord>,
) -> (
    AnyWindowHandle,
    Entity<RopyBoard>,
    async_channel::Receiver<CopyRequest>,
) {
    let (tx, rx) = async_channel::unbounded();
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_global(GlobalSettings::new(Settings::default()));
        cx.set_global(GlobalRepository::new(None));
        cx.bind_keys([
            KeyBinding::new("enter", super::ConfirmSelection, Some("!Select && !List")),
            KeyBinding::new(
                "shift-enter",
                super::ConfirmSelectionPlainText,
                Some("!Select && !List"),
            ),
            KeyBinding::new("escape", super::Hide, Some("!Select && !List")),
            KeyBinding::new("down", super::SelectNext, Some("!Select && !List")),
        ]);
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

fn record(id: u64) -> ClipboardRecord {
    ClipboardRecord::new(
        id,
        format!("record {id}"),
        chrono::Local::now(),
        crate::repository::ContentType::Text,
    )
    .pinned(true)
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
        for (button, panel) in [
            ("help-button", ActivePanel::Help),
            ("about-button", ActivePanel::About),
            ("settings-button", ActivePanel::Settings),
        ] {
            board.update(cx, |board, cx| {
                board.active_panel = ActivePanel::ClipboardList;
                cx.notify();
            });
            window.render_frame(cx);
            window.click(button, cx);
            assert_eq!(board.read(cx).active_panel, panel);
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

#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

use gpui_kit::{
    AnyWindowHandle, AppContext, Entity, Focusable, KeyBinding, TestAppContext,
    component::WindowExt, test::TestWindowExt,
};

use super::{ActivePanel, RopyBoard, filtering::ClearConfirmAction};
use crate::{
    clipboard::{ClipboardWriteError, CopyRequest, CopyTracker},
    config::{ConfirmMode, Settings},
    gui::{repository::GlobalRepository, settings::GlobalSettings},
    i18n::I18n,
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
        cx.set_global(I18n::default());
        cx.set_global(GlobalRepository::new(None));
        cx.bind_keys([
            KeyBinding::new("enter", super::ConfirmSelection, None),
            KeyBinding::new("shift-enter", super::ConfirmSelectionPlainText, None),
            KeyBinding::new("escape", super::Hide, None),
            KeyBinding::new("down", super::SelectNext, None),
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
    ClipboardRecord {
        id,
        content: format!("record {id}"),
        created_at: chrono::Local::now(),
        content_type: crate::repository::models::ContentType::Text,
        pinned: true,
        rich_text_meta: None,
    }
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
            Some(I18n::translate(cx, "repository_unavailable").as_str())
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

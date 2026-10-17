use super::*;
use gpui_kit::{App, AsKeystroke, KeyBinding, ScrollDelta, SharedString, Window, point, px};

fn open_editor(
    cx: &mut TestAppContext,
    config_file: std::path::PathBuf,
) -> (AnyWindowHandle, Entity<RopyBoard>) {
    let (handle, board, _) = open_board(cx, vec![record(1)]);
    cx.update(|cx| {
        cx.set_global(GlobalSettings::at_path(
            Settings::default(),
            Some(config_file),
        ));
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.click("help-button", cx);
        window.click("customize-shortcuts", cx);
    })
    .unwrap();
    (handle, board)
}

#[expect(clippy::panic, reason = "fail when a UI control cannot be reached")]
fn click_row_control(window: &mut Window, id: &str, cx: &mut App) {
    // Use actual scrolling/hit testing; rows below the viewport are not clickable.
    let id = SharedString::from(id.to_owned());
    for _ in 0..30 {
        window.render_frame(cx);
        let viewport = window.find("shortcuts-content").bounds();
        let target = window.find(id.clone()).bounds();
        if target.top() >= viewport.top() && target.bottom() <= viewport.bottom() {
            window.click(id, cx);
            return;
        }
        let delta = if target.top() < viewport.top() {
            100.
        } else {
            -100.
        };
        window.scroll(
            "shortcuts-content",
            ScrollDelta::Pixels(point(px(0.), px(delta))),
            cx,
        );
    }
    panic!("could not reveal {id}");
}

#[gpui_kit::test]
fn test_shortcuts_apply_persists_alias_removal_and_refreshes_help(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (handle, board) = open_editor(cx, dir.path().join("config.toml"));
    cx.update_window(handle, |_, window, cx| {
        // A binding registered after startup must survive repeated Apply operations.
        cx.bind_keys([KeyBinding::new(
            "f12",
            super::super::actions::FocusSearch,
            Some("LaterComponent"),
        )]);
        let original_count = cx.key_bindings().borrow().bindings().len();
        click_row_control(window, "shortcuts-remove-delete_record-d", cx);
        assert!(GlobalSettings::read(cx, |s| s
            .keybindings
            .history
            .is_empty()));
        window.click("shortcuts-apply", cx);
        assert_eq!(
            GlobalSettings::read(cx, |s| s.keybindings.history["delete_record"].clone()),
            ["delete"]
        );
        assert_eq!(
            cx.key_bindings().borrow().bindings().len(),
            original_count - 1
        );
        assert!(
            cx.key_bindings()
                .borrow()
                .bindings()
                .any(|binding| binding.keystrokes()[0].as_keystroke().unparse() == "f12")
        );
        window.click("shortcuts-back", cx);
        let label = window
            .find("board::DeleteRecord")
            .label()
            .unwrap()
            .to_string();
        assert!(!label.contains(" / D"), "{label}");
        window.press("escape", cx);
        window.press("d", cx);
        assert!(!board.read(cx).ui_state.delete_confirm_visible());
        window.press("delete", cx);
        assert_eq!(board.read(cx).pending_delete_id, Some(1));
    })
    .unwrap();
    let loaded = Settings::load_from_dir(dir.path()).unwrap();
    assert_eq!(loaded.keybindings.history["delete_record"], ["delete"]);
}

#[gpui_kit::test]
fn test_shortcuts_conflicting_draft_can_swap_bindings_before_apply(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (handle, board) = open_editor(cx, dir.path().join("config.toml"));
    cx.update_window(handle, |_, window, cx| {
        click_row_control(window, "shortcuts-key-preview_selection-space", cx);
        window.press("/", cx);
        window.click("shortcuts-apply", cx);
        assert!(GlobalSettings::read(cx, |s| s
            .keybindings
            .history
            .is_empty()));
        let error = window
            .find("shortcuts-error-focus_search")
            .label()
            .unwrap()
            .to_string();
        assert!(
            error.contains(t!("help_toggle_preview").as_ref()),
            "{error}"
        );
        click_row_control(window, "shortcuts-key-focus_search-/", cx);
        window.press("space", cx);
        window.click("shortcuts-apply", cx);
        window.click("shortcuts-back", cx);
        window.press("escape", cx);
        window.press("space", cx);
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
fn test_shortcuts_recording_handles_delete_enter_escape_and_tab(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let (handle, board) = open_editor(cx, dir.path().join("config.toml"));
    cx.update_window(handle, |_, window, cx| {
        for key in ["delete", "backspace", "enter"] {
            click_row_control(window, "shortcuts-add-focus_search", cx);
            window.press(key, cx);
            assert_eq!(board.read(cx).active_panel, ActivePanel::Shortcuts);
            assert!(!board.read(cx).ui_state.delete_confirm_visible());
            assert!(
                board
                    .read(cx)
                    .shortcut_editor
                    .as_ref()
                    .unwrap()
                    .draft
                    .history["focus_search"]
                    .contains(&key.to_owned())
            );
        }
        let draft = board
            .read(cx)
            .shortcut_editor
            .as_ref()
            .unwrap()
            .draft
            .clone();
        for key in ["escape", "tab"] {
            click_row_control(window, "shortcuts-add-focus_search", cx);
            window.press(key, cx);
            let editor = board.read(cx).shortcut_editor.as_ref().unwrap();
            assert!(editor.recording.is_none());
            assert_eq!(editor.draft, draft);
            assert_eq!(board.read(cx).active_panel, ActivePanel::Shortcuts);
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_shortcuts_failed_save_keeps_live_bindings_and_draft(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let blocked = dir.path().join("file-not-directory");
    std::fs::write(&blocked, "keep").unwrap();
    let (handle, board) = open_editor(cx, blocked.join("config.toml"));
    cx.update_window(handle, |_, window, cx| {
        let count = cx.key_bindings().borrow().bindings().len();
        click_row_control(window, "shortcuts-remove-delete_record-d", cx);
        window.click("shortcuts-apply", cx);
        assert!(window.find("shortcuts-save-error").label().is_some());
        assert!(board.read(cx).shortcut_editor.as_ref().unwrap().is_dirty());
        assert!(GlobalSettings::read(cx, |s| s
            .keybindings
            .history
            .is_empty()));
        assert_eq!(cx.key_bindings().borrow().bindings().len(), count);
        window.click("shortcuts-back", cx);
        window.click("cancel", cx);
        assert!(board.read(cx).shortcut_editor.as_ref().unwrap().is_dirty());
        window.press("escape", cx);
        window.click("ok", cx);
        window.press("escape", cx);
        window.press("d", cx);
        assert!(board.read(cx).ui_state.delete_confirm_visible());
    })
    .unwrap();
    assert_eq!(std::fs::read_to_string(blocked).unwrap(), "keep");
}

#[gpui_kit::test]
fn test_shortcuts_restoring_defaults_removes_overrides_without_growing_keymap(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let (handle, _) = open_editor(cx, dir.path().join("config.toml"));
    cx.update_window(handle, |_, window, cx| {
        let count = cx.key_bindings().borrow().bindings().len();
        for _ in 0..2 {
            click_row_control(window, "shortcuts-remove-focus_search-/", cx);
            window.click("shortcuts-apply", cx);
            assert!(GlobalSettings::read(cx, |s| {
                s.keybindings.history["focus_search"].is_empty()
            }));
            window.click("shortcuts-defaults", cx);
            window.click("shortcuts-apply", cx);
            assert!(GlobalSettings::read(cx, |s| s
                .keybindings
                .history
                .is_empty()));
            assert_eq!(cx.key_bindings().borrow().bindings().len(), count);
        }
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_shortcuts_editor_repairs_unknown_config_without_restart(cx: &mut TestAppContext) {
    let dir = tempfile::tempdir().unwrap();
    let mut settings = Settings::default();
    settings
        .keybindings
        .history
        .insert("unknown_action".into(), vec!["x".into()]);
    let (handle, board, _) = open_board_with_settings(cx, vec![record(1)], settings.clone());
    cx.update(|cx| {
        cx.set_global(GlobalSettings::at_path(
            settings,
            Some(dir.path().join("config.toml")),
        ));
    });
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        window.press("delete", cx);
        assert!(!board.read(cx).ui_state.delete_confirm_visible());
        window.click("help-button", cx);
        window.click("customize-shortcuts", cx);
        click_row_control(window, "shortcuts-remove-unknown-unknown_action", cx);
        window.click("shortcuts-apply", cx);
        window.click("shortcuts-back", cx);
        assert!(window.try_find("keybindings-error").is_none());
        window.press("escape", cx);
        window.press("delete", cx);
        assert!(board.read(cx).ui_state.delete_confirm_visible());
    })
    .unwrap();
    assert!(
        Settings::load_from_dir(dir.path())
            .unwrap()
            .keybindings
            .history
            .is_empty()
    );
}

#[gpui_kit::test]
fn test_shortcuts_small_window_keeps_footer_visible_and_recreated_window_keeps_draft(
    cx: &mut TestAppContext,
) {
    let dir = tempfile::tempdir().unwrap();
    let (handle, board) = open_editor(cx, dir.path().join("config.toml"));
    let repository =
        ClipboardRepository::open(&dir.path().join("db.redb"), dir.path().join("images")).unwrap();
    cx.update(|cx| cx.set_global(GlobalRepository::new(Some(Arc::new(repository)))));
    cx.simulate_window_resize(handle, gpui_kit::size(px(400.), px(550.)));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let apply = window.find("shortcuts-apply").bounds();
        assert!(apply.right() <= px(400.) && apply.bottom() <= px(550.));
        click_row_control(window, "shortcuts-remove-delete_record-d", cx);
        assert_eq!(window.find("shortcuts-apply").bounds(), apply);
        click_row_control(window, "shortcuts-add-focus_search", cx);
        board.update(cx, |board, cx| board.attach_window(window, cx));
        let editor = board.read(cx).shortcut_editor.as_ref().unwrap();
        assert!(editor.recording.is_none());
        assert!(editor.is_dirty());
        assert_eq!(editor.draft.history["delete_record"], ["delete"]);
        assert_eq!(board.read(cx).active_panel, ActivePanel::Shortcuts);
        window.render_frame(cx);
        window.click("shortcuts-revert", cx);
        window.click("shortcuts-back", cx);
        assert_eq!(board.read(cx).active_panel, ActivePanel::Help);
    })
    .unwrap();
}

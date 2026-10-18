use super::*;
use crate::gui::theme::{ThemeCatalog, ThemeId};

#[gpui_kit::test]
fn test_theme_workspace_new_draft_cancel_keeps_live_theme_and_disk(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update(|cx| {
        cx.set_global(GlobalSettings::at_path(
            Settings::default(),
            Some(directory.path().join("config.toml")),
        ));
    });
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| board.open_settings_panel(window, cx));
        window.render_frame(cx);
        window.click("theme-select", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("theme-new", cx);
        assert!(window.find("theme-save").visible());
        window.click("theme-cancel", cx);
        assert!(window.find("theme-new").visible());
        assert_eq!(
            GlobalSettings::read(cx, |settings| settings.theme.clone()),
            ThemeId::default()
        );
    })
    .unwrap();
    assert!(!directory.path().join("config.toml").exists());
    assert!(!directory.path().join("themes").exists());
}

#[gpui_kit::test]
fn test_user_theme_selection_reload_and_failure_preserve_saved_choice(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let themes = directory.path().join("themes");
    std::fs::create_dir(&themes).unwrap();
    let path = themes.join("midnight.toml");
    let source = include_str!("../../../../assets/themes/ropy-dark.toml");
    std::fs::write(&path, source.replace("#2d2d2d", "#123456")).unwrap();
    cx.update(|cx| cx.set_global(ThemeCatalog::default().reload(&themes)));
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update(|cx| {
        cx.set_global(GlobalSettings::at_path(
            Settings::default(),
            Some(directory.path().join("config.toml")),
        ));
    });
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| board.open_settings_panel(window, cx));
        window.render_frame(cx);
        window.render_frame(cx);
        window.click("theme-select", cx);
        window.render_frame(cx);
        assert_eq!(window.find("theme-select").expanded(), Some(true));
        // The user namespace sorts after bundled IDs. Exercise the real Select keyboard path.
        window.press("down", cx);
        window.render_frame(cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, _, cx| {
        assert_eq!(
            GlobalSettings::read(cx, |settings| settings.theme.code().to_owned()),
            "user:midnight"
        );
        assert_eq!(
            gpui_kit::component::Theme::global(cx).background,
            gpui_kit::rgb(0x0012_3456).into()
        );
    })
    .unwrap();
    assert_eq!(
        Settings::load_from_dir(directory.path())
            .unwrap()
            .theme
            .code(),
        "user:midnight"
    );
    cx.update_window(handle, |_, window, cx| {
        window.click("theme-select", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    std::fs::write(&path, source.replace("#2d2d2d", "#234567")).unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.click("themes-more", cx);
        window.press("down", cx);
        window.press("down", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        assert!(!board.read(cx).settings_editor.theme.busy);
        assert_eq!(
            gpui_kit::component::Theme::global(cx).background,
            gpui_kit::rgb(0x0023_4567).into()
        );
        std::fs::write(&path, "invalid TOML").unwrap();
        window.click("themes-more", cx);
        window.press("down", cx);
        window.press("down", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(ThemeCatalog::global(cx).diagnostics.len(), 1);
        assert_eq!(
            gpui_kit::component::Theme::global(cx).background,
            gpui_kit::rgb(0x0023_4567).into()
        );
        assert_eq!(
            GlobalSettings::read(cx, |settings| settings.theme.clone()),
            ThemeId::new("user:midnight")
        );
        assert!(window.find("themes-more").visible());
    })
    .unwrap();
    assert_eq!(
        Settings::load_from_dir(directory.path())
            .unwrap()
            .theme
            .code(),
        "user:midnight"
    );
}

#[gpui_kit::test]
fn test_user_theme_export_writes_complete_template_without_changing_selection(
    cx: &mut TestAppContext,
) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update(|cx| {
        cx.set_global(GlobalSettings::at_path(
            Settings::default(),
            Some(directory.path().join("config.toml")),
        ));
    });
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| board.open_settings_panel(window, cx));
        window.render_frame(cx);
        window.click("theme-select", cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.click("themes-more", cx);
        window.render_frame(cx);
        window.press("down", cx);
        window.press("enter", cx);
    })
    .unwrap();
    cx.run_until_parked();
    assert!(cx.did_prompt_for_new_path());
    cx.simulate_new_path_selection(|folder| Some(folder.join("exported.toml")));
    cx.run_until_parked();
    let source = std::fs::read_to_string(directory.path().join("themes/exported.toml")).unwrap();
    assert_eq!(
        source,
        include_str!("../../../../assets/themes/ropy-light.toml")
    );
    let catalog = ThemeCatalog::default().reload(&directory.path().join("themes"));
    assert!(catalog.get(&ThemeId::new("user:exported")).is_some());
    cx.update(|cx| {
        assert!(!board.read(cx).settings_editor.theme.busy);
        assert!(board.read(cx).settings_editor.theme.message.is_none());
        assert_eq!(
            GlobalSettings::read(cx, |settings| settings.theme.clone()),
            ThemeId::default()
        );
    });
    assert!(!directory.path().join("config.toml").exists());
}

#[gpui_kit::test]
fn test_theme_workspace_save_failure_preserves_draft_then_retry_persists(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("config.toml");
    std::fs::create_dir(&config).unwrap();
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update(|cx| {
        cx.set_global(GlobalSettings::at_path(
            Settings::default(),
            Some(config.clone()),
        ));
    });
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| board.open_themes(window, cx));
        window.click("theme-new", cx);
        let picker = board
            .read(cx)
            .theme_workspace
            .as_ref()
            .unwrap()
            .draft
            .as_ref()
            .unwrap()
            .colors[0]
            .picker
            .clone();
        picker.update(cx, |picker, cx| {
            picker.select_color(gpui_kit::rgb(0x0012_3456).into(), window, cx);
        });
        board.update(cx, |board, cx| board.attach_window(window, cx));
        window.click("theme-save", cx);
        assert!(window.find("theme-save").visible());
        assert!(
            board
                .read(cx)
                .theme_workspace
                .as_ref()
                .unwrap()
                .error
                .is_some()
        );
        assert_eq!(
            GlobalSettings::read(cx, |settings| settings.theme.clone()),
            ThemeId::default()
        );
    })
    .unwrap();
    assert_eq!(
        std::fs::read_dir(directory.path().join("themes"))
            .unwrap()
            .count(),
        0
    );
    std::fs::remove_dir(&config).unwrap();
    cx.update_window(handle, |_, window, cx| {
        window.click("theme-save", cx);
    })
    .unwrap();
    cx.run_until_parked();
    let settings = Settings::load_from_dir(directory.path()).unwrap();
    assert!(settings.theme.code().starts_with("user:custom-"));
    let catalog = ThemeCatalog::default().reload(&directory.path().join("themes"));
    assert_eq!(catalog.diagnostics, Vec::<String>::new());
    assert_eq!(
        catalog.get(&settings.theme).unwrap().palette().background,
        0x0012_3456
    );
    cx.update_window(handle, |_, window, cx| {
        assert!(window.find("theme-new").visible());
        assert_eq!(
            GlobalSettings::read(cx, |settings| settings.theme.clone()),
            settings.theme
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_theme_workspace_selection_previews_until_use(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, board, _) = open_board(cx, vec![]);
    cx.update(|cx| {
        cx.set_global(GlobalSettings::at_path(
            Settings::default(),
            Some(directory.path().join("config.toml")),
        ));
    });
    cx.update_window(handle, |_, window, cx| {
        board.update(cx, |board, cx| board.open_themes(window, cx));
        let background = gpui_kit::component::Theme::global(cx).background;
        window.click("theme-choice-ropy-dark", cx);
        assert_eq!(
            gpui_kit::component::Theme::global(cx).background,
            background
        );
        assert_eq!(
            GlobalSettings::read(cx, |settings| settings.theme.clone()),
            ThemeId::default()
        );
        window.click("theme-use", cx);
        assert_eq!(
            GlobalSettings::read(cx, |settings| settings.theme.code().to_owned()),
            "ropy-dark"
        );
        assert_ne!(
            gpui_kit::component::Theme::global(cx).background,
            background
        );
    })
    .unwrap();
}

use super::*;
use gpui_kit::{
    MouseButton, ScrollDelta, point, px, size,
    test::{TestAppContextExt, TestEventExt},
};

#[gpui_kit::test]
fn test_preview_hold_opens_bounded_window_and_release_in_either_window_closes_it(
    cx: &mut TestAppContext,
) {
    let (main, board, copies) = open_board(cx, vec![record(1)]);
    cx.simulate_window_resize(main, size(px(400.), px(550.)));
    for release_in_preview in [false, true] {
        cx.update_window(main, |_, window, cx| {
            board.update(cx, |board, _| {
                board.settings_editor.panel_state.space_preview_enabled = true;
                board.settings_editor.panel_state.hover_preview_enabled = false;
            });
            window.key_down("space", false, cx);
            window.key_down("space", true, cx);
        })
        .unwrap();
        cx.run_until_parked();
        let preview = cx.update(|cx| {
            assert_eq!(cx.windows().len(), 2, "hold must open one native preview");
            cx.windows()
                .into_iter()
                .find(|handle| *handle != main)
                .unwrap()
        });
        let main_size = cx
            .update_window(main, |_, window, _| window.bounds().size)
            .unwrap();
        cx.update_window(preview, |_, window, cx| {
            let work_area = window.display(cx).unwrap().visible_bounds();
            let bounds = window.bounds();
            assert_eq!(bounds.size, main_size);
            assert!(bounds.size.width <= work_area.size.width);
            assert!(bounds.size.height <= work_area.size.height);
            assert!((bounds.center().x - work_area.center().x).abs() < px(1.));
            assert!((bounds.center().y - work_area.center().y).abs() < px(1.));
        })
        .unwrap();
        cx.update_window(
            if release_in_preview { preview } else { main },
            |_, window, cx| {
                window.key_up("space", cx);
            },
        )
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| assert_eq!(cx.windows(), vec![main]));
        assert!(copies.is_empty());
    }
}

#[gpui_kit::test]
fn test_preview_escape_closes_only_preview_and_repeat_does_not_reopen(cx: &mut TestAppContext) {
    let (main, board, _) = open_board(cx, vec![record(1)]);
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, _| {
            board.settings_editor.panel_state.space_preview_enabled = true;
        });
        window.key_down("space", false, cx);
        window.press("escape", cx);
        window.key_down("space", true, cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows(), vec![main]));
}

#[expect(
    clippy::future_not_send,
    reason = "GPUI UI tests run on the foreground executor"
)]
async fn open_image_preview(
    cx: &mut TestAppContext,
) -> (tempfile::TempDir, AnyWindowHandle, AnyWindowHandle) {
    let fixture = tempfile::tempdir().unwrap();
    let path = fixture.path().join("image.png");
    image::RgbaImage::new(2400, 1200).save(&path).unwrap();
    let image = ClipboardRecord::new(
        1,
        path.to_string_lossy().into_owned(),
        chrono::Local::now(),
        crate::repository::ContentType::Image,
    );
    let (main, board, _) = open_board(cx, vec![image]);
    cx.simulate_window_resize(main, size(px(400.), px(550.)));
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, _| {
            board.settings_editor.panel_state.space_preview_enabled = true;
        });
        window.key_down("space", false, cx);
    })
    .unwrap();
    let preview = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .find(|handle| *handle != main)
            .unwrap()
    });
    cx.wait_for(preview, Duration::from_secs(3), |window, _| {
        window.try_find("preview-image").is_some()
    })
    .await;
    (fixture, main, preview)
}

#[gpui_kit::test]
#[expect(
    clippy::future_not_send,
    reason = "GPUI UI tests run on the foreground executor"
)]
async fn test_preview_image_wheel_and_drag_change_rendered_geometry(cx: &mut TestAppContext) {
    let (_fixture, main, preview) = open_image_preview(cx).await;
    cx.update_window(preview, |_, window, cx| {
        let fitted = window.find("preview-image").bounds();
        let canvas = window.find("preview-canvas").bounds();
        let header = window.find("preview-header").bounds();
        let footer = window.find("preview-footer").bounds();
        assert!(header.bottom() <= canvas.top());
        assert!(footer.top() >= canvas.bottom());
        assert!(footer.contains(&window.find("preview-fit").bounds().center()));
        assert!(footer.contains(&window.find("preview-original").bounds().center()));
        assert_eq!(window.find("preview-details").label(), Some("2400 × 1200"));
        let fitted_percent = format!(
            "{:.0}%",
            fitted.size.width / px(2400.) * window.scale_factor() * 100.
        );
        assert_eq!(
            window.find("preview-scale").label(),
            Some(fitted_percent.as_str())
        );
        assert!(fitted.size.width <= canvas.size.width);
        assert!(fitted.size.height <= canvas.size.height);
        assert!((fitted.size.width / fitted.size.height - 2.).abs() < 0.001);
        window.drag(
            canvas.center(),
            canvas.center() + point(px(60.), px(40.)),
            cx,
        );
        assert_eq!(
            window.find("preview-image").bounds(),
            fitted,
            "fitted image cannot pan into empty space"
        );
        window.click("preview-original", cx);
        let original = window.find("preview-image").bounds();
        assert_eq!(window.find("preview-scale").label(), Some("100%"));
        assert_eq!(
            original.size.map(|edge| edge * window.scale_factor()),
            size(px(2400.), px(1200.))
        );
        window.scroll(
            "preview-canvas",
            ScrollDelta::Pixels(point(px(0.), px(200.))),
            cx,
        );
        let zoomed = window.find("preview-image").bounds();
        assert_eq!(window.find("preview-scale").label(), Some("149%"));
        assert!(
            zoomed.size.width > original.size.width,
            "wheel must zoom the actual image"
        );
        window.pointer_down(canvas.center(), MouseButton::Left, cx);
        window.pointer_move(
            canvas.center() + point(px(60.), px(40.)),
            Some(MouseButton::Left),
            cx,
        );
        window.pointer_up(
            canvas.center() + point(px(60.), px(40.)),
            MouseButton::Left,
            cx,
        );
        let moved = window.find("preview-image").bounds();
        assert_eq!(moved.origin - zoomed.origin, point(px(60.), px(40.)));
        window.click("preview-fit", cx);
        assert_eq!(window.find("preview-image").bounds(), fitted);
        // The release must close even after a toolbar button owns focus.
        window.key_up("space", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows(), vec![main]));
}

#[gpui_kit::test]
#[expect(
    clippy::future_not_send,
    reason = "GPUI UI tests run on the foreground executor"
)]
async fn test_preview_scale_refreshes_after_canvas_resize(cx: &mut TestAppContext) {
    let (_fixture, main, preview) = open_image_preview(cx).await;
    cx.simulate_window_resize(preview, size(px(320.), px(550.)));
    cx.update_window(preview, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.simulate_next_frame(cx) > 0,
            "resolved image geometry must schedule a frame for the scale label"
        );
        window.render_frame(cx);
        let resized_percent = format!(
            "{:.0}%",
            window.find("preview-image").bounds().size.width / px(2400.)
                * window.scale_factor()
                * 100.
        );
        assert_eq!(
            window.find("preview-scale").label(),
            Some(resized_percent.as_str())
        );
        window.key_up("space", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows(), vec![main]));
}

#[gpui_kit::test]
fn test_preview_parent_close_removes_child_and_empty_or_disabled_history_does_not_open(
    cx: &mut TestAppContext,
) {
    let (main, board, _) = open_board(cx, vec![]);
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, _| {
            board.settings_editor.panel_state.space_preview_enabled = true;
        });
        window.key_down("space", false, cx);
        assert!(board.read(cx).preview_window.is_none());
    })
    .unwrap();
    cx.update(|cx| assert_eq!(cx.windows(), vec![main]));
    cx.update_window(main, |_, window, _| window.remove_window())
        .unwrap();

    let (main, board, _) = open_board(cx, vec![record(1)]);
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, _| {
            board.settings_editor.panel_state.space_preview_enabled = false;
        });
        window.key_down("space", false, cx);
        assert!(board.read(cx).preview_window.is_none());
        window.key_up("space", cx);
        board.update(cx, |board, _| {
            board.settings_editor.panel_state.space_preview_enabled = true;
        });
        window.key_down("space", false, cx);
    })
    .unwrap();
    cx.update(|cx| assert_eq!(cx.windows().len(), 2));
    cx.update_window(main, |_, window, _| window.remove_window())
        .unwrap();
    cx.run_until_parked();
    cx.update(|cx| {
        assert!(
            cx.windows().is_empty(),
            "closing the main window must not orphan a preview"
        );
        assert!(board.read(cx).preview_window.is_none());
    });
}

#[gpui_kit::test]
fn test_preview_focus_transfer_survives_main_deactivation_then_external_focus_closes(
    cx: &mut TestAppContext,
) {
    let (main, board, _) = open_board(cx, vec![record(1)]);
    cx.update_window(main, |_, window, cx| {
        window.activate_window();
        board.update(cx, |board, _| {
            board.settings_editor.panel_state.space_preview_enabled = true;
        });
        window.key_down("space", false, cx);
    })
    .unwrap();
    let preview = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .find(|handle| *handle != main)
            .unwrap()
    });
    gpui_kit::VisualTestContext::from_window(main, cx).deactivate_window();
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            cx.windows().len(),
            2,
            "main resigns before the preview becomes key"
        );
    });
    cx.update_window(preview, |_, window, _| window.activate_window())
        .unwrap();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows().len(), 2));
    gpui_kit::VisualTestContext::from_window(preview, cx).deactivate_window();
    cx.executor().advance_clock(Duration::from_millis(100));
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(cx.windows(), vec![main]);
        assert!(board.read(cx).preview_window.is_none());
    });
}

#[gpui_kit::test]
fn test_preview_long_text_scrolls_to_untruncated_tail(cx: &mut TestAppContext) {
    let mut text = record(1);
    text.content = (0..200)
        .map(|ix| format!("line {ix}: {}", "long content ".repeat(20)))
        .collect::<Vec<_>>()
        .join("\n");
    let expected_tail = text.content.lines().last().unwrap().to_owned();
    let (main, board, _) = open_board(cx, vec![text]);
    cx.simulate_window_resize(main, size(px(400.), px(550.)));
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, _| {
            board.settings_editor.panel_state.space_preview_enabled = true;
        });
        window.key_down("space", false, cx);
    })
    .unwrap();
    let preview = cx.update(|cx| {
        cx.windows()
            .into_iter()
            .find(|handle| *handle != main)
            .unwrap()
    });
    cx.update_window(preview, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.try_find(("preview-line", 199_usize)).is_none(),
            "long content should be virtualized"
        );
        // Virtual-list scroll deltas are capped to the measured viewport.
        // Allow enough gestures for wrapped lines in a main-window-sized preview.
        for _ in 0..200 {
            window.scroll(
                "preview-text",
                ScrollDelta::Pixels(point(px(0.), px(-1000.))),
                cx,
            );
            if window.try_find(("preview-line", 199_usize)).is_some() {
                break;
            }
        }
        assert_eq!(
            window.find(("preview-line", 199_usize)).label(),
            Some(expected_tail.as_str())
        );
        window.key_up("space", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows(), vec![main]));
}

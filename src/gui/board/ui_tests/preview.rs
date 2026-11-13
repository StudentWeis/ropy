use super::*;
use gpui_kit::{
    MouseButton, ScrollDelta, point, px, size,
    test::{TestAppContextExt, TestEventExt},
};

fn open_board(
    cx: &mut TestAppContext,
    records: Vec<ClipboardRecord>,
) -> (
    AnyWindowHandle,
    Entity<RopyBoard>,
    async_channel::Receiver<CopyRequest>,
) {
    let result = super::open_board(cx, records);
    cx.update_window(result.0, |_, window, _| window.activate_window())
        .unwrap();
    result
}

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
        let main_bounds = cx
            .update_window(main, |_, window, _| window.bounds())
            .unwrap();
        cx.update_window(preview, |_, window, cx| {
            let work_area = window.display(cx).unwrap().visible_bounds();
            let bounds = window.bounds();
            assert!(bounds.size.width <= work_area.size.width);
            assert!(bounds.size.height <= work_area.size.height);
            assert!(bounds.left() >= work_area.left());
            assert!(bounds.right() <= work_area.right());
            assert!(bounds.top() >= work_area.top());
            assert!(bounds.bottom() <= work_area.bottom());
            assert!(
                bounds.left() >= main_bounds.right() || bounds.right() <= main_bounds.left(),
                "preview {bounds:?} should sit beside main {main_bounds:?}"
            );
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
    hover: bool,
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
    let mut settings = Settings::default();
    settings.preview.hover_preview_enabled = hover;
    settings.preview.space_preview_enabled = true;
    let (main, _, _) = open_board_with_settings(cx, vec![image], settings);
    cx.simulate_window_resize(main, size(px(400.), px(550.)));
    cx.update_window(main, |_, window, _| window.activate_window())
        .unwrap();
    cx.run_until_parked();
    cx.update_window(main, |_, window, cx| {
        if hover {
            window.hover(("record-content", 1_u64), cx);
        } else {
            window.key_down("space", false, cx);
        }
    })
    .unwrap();
    if hover {
        cx.executor().advance_clock(Duration::from_millis(400));
        cx.run_until_parked();
    }
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
    let (_fixture, main, preview) = open_image_preview(cx, false).await;
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
    let (_fixture, main, preview) = open_image_preview(cx, false).await;
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

#[gpui_kit::test]
fn test_preview_follows_main_window_origin(cx: &mut TestAppContext) {
    let (tx, _rx) = async_channel::unbounded();
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        cx.set_global(GlobalSettings::new(Settings::default()));
        cx.set_global(GlobalRepository::new(None));
        crate::gui::board::bind_application_keys(cx);
    });
    for (main_origin, expect_right_side) in [
        (point(px(40.), px(60.)), true),
        (point(px(1480.), px(60.)), false),
    ] {
        let main_bounds = gpui_kit::Bounds::new(main_origin, size(px(400.), px(550.)));
        let (main, board) = cx.update(|cx| {
            gpui_kit::open_window(
                gpui_kit::WindowOptions {
                    window_bounds: Some(gpui_kit::WindowBounds::Windowed(main_bounds)),
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    cx.new(|cx| {
                        let mut board = RopyBoard::new(
                            Arc::new(RwLock::new(vec![record(1)])),
                            Arc::new(Mutex::new(CopyTracker::default())),
                            tx.clone(),
                            window,
                            cx,
                        );
                        board.activated = true;
                        board.pinned = true;
                        board.settings_editor.panel_state.space_preview_enabled = true;
                        board
                    })
                },
            )
            .expect("open main board")
        });
        cx.update_window(main, |_, window, cx| {
            window.key_down("space", false, cx);
        })
        .unwrap();
        cx.run_until_parked();
        let preview = cx.update(|cx| {
            cx.windows()
                .into_iter()
                .find(|handle| *handle != main)
                .expect("preview window should open")
        });
        let (actual_main_bounds, expected_top) = cx
            .update_window(main, |_, window, cx| {
                let bounds = window.bounds();
                let row = board.read(cx).list_state.bounds_for_item(0).unwrap();
                (bounds, bounds.top() + row.top())
            })
            .unwrap();
        cx.update_window(preview, |_, window, _| {
            let preview_bounds = window.bounds();
            assert_eq!(preview_bounds.top(), expected_top);
            if expect_right_side {
                assert!(preview_bounds.left() > actual_main_bounds.right());
            } else {
                assert!(preview_bounds.right() < actual_main_bounds.left());
            }
        })
        .unwrap();
        cx.update_window(main, |_, window, _| window.remove_window())
            .unwrap();
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
fn test_hover_preview_pending_open_is_cancelled_on_dismiss_or_deactivation(
    cx: &mut TestAppContext,
) {
    for deactivate in [false, true] {
        let mut settings = Settings::default();
        settings.preview.hover_preview_enabled = true;
        let (main, board, _) = open_board_with_settings(cx, vec![record(1)], settings);
        cx.update_window(main, |_, window, cx| {
            window.activate_window();
            board.update(cx, |board, _| board.pinned = true);
            window.hover(("record-content", 1_u64), cx);
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(100));
        cx.run_until_parked();
        if deactivate {
            gpui_kit::VisualTestContext::from_window(main, cx).deactivate_window();
        } else {
            cx.update_window(main, |_, window, cx| window.press("escape", cx))
                .unwrap();
        }
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(400));
        cx.run_until_parked();
        cx.update(|cx| assert_eq!(cx.windows(), vec![main], "deactivate={deactivate}"));
        cx.update_window(main, |_, window, _| {
            if deactivate {
                assert!(
                    !window.is_window_active(),
                    "hover must not reactivate the parent"
                );
            }
            window.remove_window();
        })
        .unwrap();
        cx.run_until_parked();
    }
}

#[gpui_kit::test]
#[expect(
    clippy::future_not_send,
    reason = "GPUI UI tests run on the foreground executor"
)]
async fn test_hover_preview_toolbar_click_preserves_escape_dismissal(cx: &mut TestAppContext) {
    let (_fixture, main, preview) = open_image_preview(cx, true).await;
    cx.update_window(main, |_, window, _| assert!(window.is_window_active()))
        .unwrap();
    cx.update_window(preview, |_, window, cx| {
        window.activate_window();
        window.click("preview-original", cx);
        assert_eq!(window.find("preview-scale").label(), Some("100%"));
        window.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows(), vec![main]));
}

#[gpui_kit::test]
fn test_hover_preview_opens_side_window_after_delay_without_stealing_focus_and_updates_on_row_switch(
    cx: &mut TestAppContext,
) {
    let (main, board, _) = open_board(cx, vec![record(1), record(2)]);
    cx.simulate_window_resize(main, size(px(400.), px(550.)));
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.settings_editor.panel_state.hover_preview_enabled = true;
            board.on_record_hover(1, true, window, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    cx.update(|cx| {
        assert_eq!(
            cx.windows().len(),
            1,
            "hover preview must wait for the open debounce"
        );
    });

    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    let preview = cx.update(|cx| {
        assert_eq!(cx.windows().len(), 2);
        cx.windows()
            .into_iter()
            .find(|handle| *handle != main)
            .unwrap()
    });
    cx.update_window(main, |_, window, _| {
        assert!(
            window.is_window_active(),
            "hover preview must not steal focus from the main window"
        );
    })
    .unwrap();
    cx.update_window(preview, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(("preview-line", 0_usize)).label(),
            Some("record 1")
        );
    })
    .unwrap();

    // Switching to another row updates the existing hover preview in-place.
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.on_record_hover(1, false, window, cx);
            board.on_record_hover(2, true, window, cx);
        });
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows().len(), 2));
    cx.update_window(preview, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(("preview-line", 0_usize)).label(),
            Some("record 2")
        );
    })
    .unwrap();

    // Leaving the row closes the hover preview after the grace period
    // once the pointer is not inside the preview window either.
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.on_record_hover(2, false, window, cx);
            board.on_preview_hover(false, window, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(250));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows(), vec![main]));
}

#[gpui_kit::test]
fn test_hover_preview_stays_open_when_pointer_moves_into_preview_window(cx: &mut TestAppContext) {
    let (main, board, _) = open_board(cx, vec![record(1)]);
    cx.simulate_window_resize(main, size(px(400.), px(550.)));
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.settings_editor.panel_state.hover_preview_enabled = true;
            board.on_record_hover(1, true, window, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(400));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows().len(), 2));

    // Leaving the row and entering the preview window cancels the close timer.
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.on_record_hover(1, false, window, cx);
            board.on_preview_hover(true, window, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows().len(), 2));

    // Leaving the preview window closes it after the grace period.
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.on_preview_hover(false, window, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(250));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows(), vec![main]));
}

#[gpui_kit::test]
fn test_grid_hover_preview_ignores_brief_cross_over_and_switches_on_dwell(cx: &mut TestAppContext) {
    let (main, board, _) = open_board(cx, vec![record(1), record(2)]);
    cx.simulate_window_resize(main, size(px(400.), px(550.)));
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.layout_mode = crate::config::LayoutMode::Grid;
            board.settings_editor.panel_state.hover_preview_enabled = true;
            board.on_record_hover(1, true, window, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(400));
    cx.run_until_parked();
    let preview = cx.update(|cx| {
        assert_eq!(cx.windows().len(), 2);
        cx.windows()
            .into_iter()
            .find(|handle| *handle != main)
            .unwrap()
    });

    // Crossing an adjacent grid card briefly on the way to the preview window keeps record 1.
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.on_record_hover(1, false, window, cx);
            board.on_record_hover(2, true, window, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(120));
    cx.run_until_parked();
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.on_record_hover(2, false, window, cx);
            board.on_preview_hover(true, window, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(200));
    cx.run_until_parked();
    cx.update_window(preview, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(("preview-line", 0_usize)).label(),
            Some("record 1")
        );
    })
    .unwrap();

    // Dwelling on another grid card switches the preview after the grid switch delay.
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.on_preview_hover(false, window, cx);
            board.on_record_hover(2, true, window, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(300));
    cx.run_until_parked();
    let preview = cx.update(|cx| {
        assert_eq!(cx.windows().len(), 2);
        cx.windows()
            .into_iter()
            .find(|handle| *handle != main)
            .unwrap()
    });
    cx.update_window(preview, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(("preview-line", 0_usize)).label(),
            Some("record 2")
        );
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_hover_preview_aligns_vertically_with_hovered_item(cx: &mut TestAppContext) {
    let (main, board, _) = open_board(cx, vec![record(1)]);
    cx.simulate_window_resize(main, size(px(400.), px(550.)));
    let main_top = cx
        .update_window(main, |_, window, _| window.bounds().top())
        .unwrap();
    cx.update_window(main, |_, window, cx| {
        window.simulate_mouse_move(point(px(100.), px(380.)), cx);
        board.update(cx, |board, cx| {
            board.settings_editor.panel_state.hover_preview_enabled = true;
            board.on_record_hover(1, true, window, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(400));
    cx.run_until_parked();
    let preview = cx.update(|cx| {
        assert_eq!(cx.windows().len(), 2);
        cx.windows()
            .into_iter()
            .find(|handle| *handle != main)
            .unwrap()
    });
    cx.update_window(preview, |_, window, _| {
        assert_eq!(window.bounds().top(), main_top + px(360.));
    })
    .unwrap();
}

#[gpui_kit::test]
fn test_space_preview_aligns_vertically_with_selected_item_in_list_mode(cx: &mut TestAppContext) {
    let (main, board, _) = open_board(
        cx,
        vec![record(1), record(2), record(3), record(4), record(5)],
    );
    cx.simulate_window_resize(main, size(px(400.), px(550.)));
    let expected_top = cx
        .update_window(main, |_, window, cx| {
            window.render_frame(cx);
            window.bounds().top() + board.read(cx).list_state.bounds_for_item(3).unwrap().top()
        })
        .unwrap();

    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.settings_editor.panel_state.space_preview_enabled = true;
            board.selected_index = 3;
            cx.notify();
        });
        window.key_down("space", false, cx);
    })
    .unwrap();
    cx.run_until_parked();

    let preview = cx.update(|cx| {
        assert_eq!(cx.windows().len(), 2);
        cx.windows()
            .into_iter()
            .find(|handle| *handle != main)
            .unwrap()
    });
    cx.update_window(preview, |_, window, _| {
        assert_eq!(window.bounds().top(), expected_top);
    })
    .unwrap();

    cx.update_window(main, |_, window, cx| {
        window.key_up("space", cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn test_grid_space_preview_docks_next_to_selected_column_and_row(cx: &mut TestAppContext) {
    let (tx, _rx) = async_channel::unbounded();
    cx.update(|cx| {
        gpui_kit::init(cx);
        cx.set_reduce_motion(true);
        cx.set_global(GlobalSettings::new(Settings::default()));
        cx.set_global(GlobalRepository::new(None));
        crate::gui::board::bind_application_keys(cx);
    });
    let main_bounds = gpui_kit::Bounds::new(point(px(760.), px(265.)), size(px(400.), px(550.)));
    let (main, board) = cx.update(|cx| {
        gpui_kit::open_window(
            gpui_kit::WindowOptions {
                window_bounds: Some(gpui_kit::WindowBounds::Windowed(main_bounds)),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    let mut board = RopyBoard::new(
                        Arc::new(RwLock::new(vec![
                            record(1),
                            record(2),
                            record(3),
                            record(4),
                        ])),
                        Arc::new(Mutex::new(CopyTracker::default())),
                        tx,
                        window,
                        cx,
                    );
                    board.activated = true;
                    board.pinned = true;
                    board.layout_mode = crate::config::LayoutMode::Grid;
                    board.settings_editor.panel_state.space_preview_enabled = true;
                    board.selected_index = 0;
                    board
                })
            },
        )
        .expect("open main board")
    });
    cx.update_window(main, |_, window, cx| {
        window.render_frame(cx);
        window.key_down("space", false, cx);
    })
    .unwrap();
    cx.run_until_parked();

    let preview_left = cx.update(|cx| {
        assert_eq!(cx.windows().len(), 2);
        cx.windows()
            .into_iter()
            .find(|handle| *handle != main)
            .unwrap()
    });
    cx.update_window(preview_left, |_, window, _| {
        assert!(window.bounds().right() < main_bounds.left());
    })
    .unwrap();

    cx.update_window(main, |_, window, cx| {
        window.key_up("space", cx);
    })
    .unwrap();
    cx.run_until_parked();

    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.selected_index = 1;
            cx.notify();
        });
        window.render_frame(cx);
        window.key_down("space", false, cx);
    })
    .unwrap();
    cx.run_until_parked();

    let preview_right = cx.update(|cx| {
        assert_eq!(cx.windows().len(), 2);
        cx.windows()
            .into_iter()
            .find(|handle| *handle != main)
            .unwrap()
    });
    cx.update_window(preview_right, |_, window, _| {
        assert!(window.bounds().left() > main_bounds.right());
    })
    .unwrap();

    cx.update_window(main, |_, window, cx| {
        window.key_up("space", cx);
    })
    .unwrap();
    cx.run_until_parked();
}

#[gpui_kit::test]
fn test_hover_preview_is_replaced_by_hold_preview(cx: &mut TestAppContext) {
    let (main, board, _) = open_board(cx, vec![record(1), record(2)]);
    cx.simulate_window_resize(main, size(px(400.), px(550.)));
    cx.run_until_parked();
    cx.update_window(main, |_, window, cx| {
        board.update(cx, |board, cx| {
            board.settings_editor.panel_state.hover_preview_enabled = true;
            board.settings_editor.panel_state.space_preview_enabled = true;
            board.on_record_hover(1, true, window, cx);
        });
    })
    .unwrap();
    cx.executor().advance_clock(Duration::from_millis(400));
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows().len(), 2));

    // Pressing Space replaces the hover preview with a hold preview for the selected row (2).
    cx.update_window(main, |_, window, cx| {
        assert!(
            board.read(cx).focus_handle.is_focused(window),
            "main board should still have focus"
        );
        window.key_down("space", false, cx);
        assert!(
            board.read(cx).is_hold_preview_active(),
            "space should replace hover preview with hold preview"
        );
    })
    .unwrap();
    cx.run_until_parked();
    let preview = cx.update(|cx| {
        assert_eq!(cx.windows().len(), 2);
        cx.windows()
            .into_iter()
            .find(|handle| *handle != main)
            .unwrap()
    });
    cx.update_window(preview, |_, window, cx| {
        window.render_frame(cx);
        assert_eq!(
            window.find(("preview-line", 0_usize)).label(),
            Some("record 2")
        );
        window.key_up("space", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update(|cx| assert_eq!(cx.windows(), vec![main]));
}

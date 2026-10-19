use gpui_kit::{
    Context, StatefulInteractiveElement,
    component::{
        ActiveTheme, Icon, IndexPath, Sizable, StyledExt,
        button::{Button, ButtonVariants},
        h_flex,
        input::Input,
        select::Select,
        separator::Separator,
        slider::Slider,
        switch::Switch,
        v_flex,
    },
    div,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
    px, rems,
};
use rust_i18n::t;

use crate::gui::{
    board::{RopyBoard, filtering::ClearConfirmAction},
    panel::common::{panel_back_button, panel_header_with_back},
    settings::GlobalSettings,
};

mod theme;

fn settings_row<C: IntoElement>(
    label: impl IntoElement,
    control: C,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    h_flex()
        .justify_between()
        .items_center()
        .w_full()
        .gap_3()
        .py_2()
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(label),
        )
        .child(control)
}

fn settings_control(control: impl IntoElement) -> gpui_kit::Div {
    div().w(rems(11.)).flex_shrink_0().child(control)
}

fn settings_section_header(
    label: impl Into<gpui_kit::SharedString>,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    div()
        .w_full()
        .pt_3()
        .pb_1()
        .text_xs()
        .font_medium()
        .text_color(cx.theme().muted_foreground)
        .child(label.into())
}

fn settings_card(cx: &Context<'_, RopyBoard>) -> gpui_kit::Div {
    v_flex()
        .w_full()
        .mt_3()
        .px_5()
        .rounded_xl()
        .bg(cx.theme().secondary)
}

fn section_card(
    title: impl Into<gpui_kit::SharedString>,
    rows: Vec<gpui_kit::AnyElement>,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let mut card = settings_card(cx).child(settings_section_header(title, cx));
    let last_index = rows.len().saturating_sub(1);
    for (index, row) in rows.into_iter().enumerate() {
        card = card.child(row);
        if index < last_index {
            card = card.child(Separator::horizontal());
        }
    }
    card
}

pub(crate) fn render_settings_content(
    board: &RopyBoard,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let header = render_settings_header(cx);

    let appearance_card = section_card(
        t!("settings_section_appearance"),
        vec![
            theme::render_theme_settings(board, cx).into_any_element(),
            render_language_row(board, cx).into_any_element(),
            render_layout_row(board, cx).into_any_element(),
            render_window_opacity_row(board, cx).into_any_element(),
        ],
        cx,
    );

    let behavior_card = section_card(
        t!("settings_section_behavior"),
        vec![
            render_activation_key_row(board, cx).into_any_element(),
            settings_row(
                t!("shortcuts_title"),
                Button::new("settings-shortcuts")
                    .small()
                    .label(t!("shortcuts_customize"))
                    .on_click(cx.listener(|board, _, window, cx| board.open_shortcuts(window, cx))),
                cx,
            )
            .into_any_element(),
            render_confirm_mode_row(board, cx).into_any_element(),
            render_hover_preview_row(board, cx).into_any_element(),
            render_space_preview_row(board, cx).into_any_element(),
            render_autostart_row(board, cx).into_any_element(),
        ],
        cx,
    );

    let storage_card = section_card(
        t!("settings_section_storage"),
        vec![
            render_max_history_row(board, cx).into_any_element(),
            render_max_storage_row(board, cx).into_any_element(),
            render_clear_history_row(cx).into_any_element(),
        ],
        cx,
    );

    let about_card = section_card(
        t!("settings_section_about"),
        vec![
            render_auto_check_row(board, cx).into_any_element(),
            render_include_prerelease_row(board, cx).into_any_element(),
            render_open_dirs_row(cx).into_any_element(),
        ],
        cx,
    );

    v_flex().size_full().child(header).child(
        v_flex()
            .id("settings-content")
            .overflow_y_scroll()
            .size_full()
            .flex_1()
            .px_2()
            .pb_4()
            .child(appearance_card)
            .child(behavior_card)
            .child(storage_card)
            .child(about_card),
    )
}

fn render_settings_header(cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    panel_header_with_back(
        t!("settings_title"),
        panel_back_button("cancel-button")
            .on_click(cx.listener(|board, _click_event, window, cx| {
                reset_settings_dialog(board, window, cx);
            }))
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            }),
        cx,
    )
}

fn render_language_row(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    settings_row(
        t!("settings_language"),
        settings_control(
            Select::new(&board.settings_editor.language_select)
                .accessibility_label(t!("settings_language")),
        ),
        cx,
    )
}

fn render_layout_row(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    settings_row(
        t!("settings_layout"),
        settings_control(
            Select::new(&board.settings_editor.layout_select)
                .accessibility_label(t!("settings_layout")),
        ),
        cx,
    )
}

fn render_activation_key_row(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    settings_row(
        t!("settings_activation_key"),
        render_hotkey_controls(board, cx),
        cx,
    )
}

fn render_window_opacity_row(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    settings_row(
        t!("settings_window_opacity"),
        h_flex()
            .w(rems(11.))
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .child(
                Slider::new(&board.settings_editor.settings_window_opacity_slider)
                    .accessibility_label(t!("settings_window_opacity"))
                    .flex_1()
                    .bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
                    .opacity(
                        if board
                            .settings_editor
                            .panel_state
                            .window_opacity_slider_visible
                        {
                            1.0_f32
                        } else {
                            0.0_f32
                        },
                    ),
            )
            .child(
                div()
                    .w(px(40.0))
                    .text_right()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{}%", board.settings_editor.window_opacity_percent)),
            )
            .on_mouse_up(
                gpui_kit::MouseButton::Left,
                cx.listener(|board, _, window, cx| {
                    board.save_window_opacity(window, cx);
                }),
            )
            .on_mouse_up_out(
                gpui_kit::MouseButton::Left,
                cx.listener(|board, _, window, cx| {
                    board.save_window_opacity(window, cx);
                }),
            ),
        cx,
    )
}

fn render_hotkey_controls(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let is_dirty = board.has_pending_hotkey(cx);

    let border_color = if board.settings_editor.hotkey.recording {
        cx.theme().ring
    } else {
        cx.theme().input
    };

    let record_tooltip = if board.settings_editor.hotkey.recording {
        t!("settings_hotkey_record_hint")
    } else {
        t!("settings_hotkey_record")
    };

    let record_button = if board.settings_editor.hotkey.recording {
        Button::new("hotkey-record-button").primary()
    } else {
        Button::new("hotkey-record-button")
            .ghost()
            .opacity(0.72_f32)
    };

    let save_button = if is_dirty {
        Button::new("hotkey-save-button").primary()
    } else {
        Button::new("hotkey-save-button").ghost().opacity(0.55_f32)
    };

    let actions = h_flex()
        .gap_1()
        .child(
            record_button
                .xsmall()
                .compact()
                .icon(Icon::empty().path("icons/hotkey-record.svg"))
                .accessibility_label(record_tooltip.clone())
                .tooltip(record_tooltip)
                .on_click(cx.listener(|board, _, window, cx| {
                    if board.settings_editor.hotkey.recording {
                        board.cancel_hotkey_recording(window, cx);
                    } else {
                        board.start_hotkey_recording(window, cx);
                    }
                }))
                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                }),
        )
        .child(
            save_button
                .xsmall()
                .compact()
                .icon(Icon::empty().path("icons/check.svg"))
                .accessibility_label(t!("settings_hotkey_save"))
                .tooltip(t!("settings_hotkey_save"))
                .on_click(cx.listener(|board, _, window, cx| {
                    board.save_hotkey(cx, window);
                }))
                .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                }),
        );
    settings_control(
        Input::new(&board.settings_editor.settings_activation_key_input)
            .aria_label(t!("settings_activation_key"))
            .border_color(border_color)
            .suffix(actions),
    )
}

fn render_max_history_row(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let is_dirty = board.has_pending_max_history(cx);
    let save_button = if is_dirty {
        Button::new("max-history-save-button").primary()
    } else {
        Button::new("max-history-save-button")
            .ghost()
            .opacity(0.55_f32)
    };

    settings_row(
        t!("settings_max_history"),
        h_flex()
            .w(px(100.0))
            .items_center()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .px_2()
            .py_1()
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&board.settings_editor.settings_max_history_input)
                        .aria_label(t!("settings_max_history"))
                        .appearance(false)
                        .px_2()
                        .py_1(),
                ),
            )
            .child(
                save_button
                    .accessibility_label(t!("settings_max_history_save"))
                    .xsmall()
                    .compact()
                    .rounded(px(4.0))
                    .icon(Icon::empty().path("icons/check.svg").size(px(14.0)))
                    .on_click(cx.listener(|board, _, window, cx| {
                        board.save_max_history(cx, window);
                    }))
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    }),
            ),
        cx,
    )
}

fn render_max_storage_row(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let is_dirty = board.has_pending_max_storage(cx);
    let save_button = if is_dirty {
        Button::new("max-storage-save-button").primary()
    } else {
        Button::new("max-storage-save-button")
            .ghost()
            .opacity(0.55_f32)
    };

    settings_row(
        t!("settings_max_storage"),
        h_flex()
            .w(px(100.0))
            .items_center()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .px_2()
            .py_1()
            .child(
                div().flex_1().min_w_0().child(
                    Input::new(&board.settings_editor.settings_max_storage_input)
                        .aria_label(t!("settings_max_storage"))
                        .appearance(false)
                        .px_2()
                        .py_1(),
                ),
            )
            .child(
                save_button
                    .accessibility_label(t!("settings_max_storage_save"))
                    .xsmall()
                    .compact()
                    .rounded(px(4.0))
                    .icon(Icon::empty().path("icons/check.svg").size(px(14.0)))
                    .on_click(cx.listener(|board, _, window, cx| {
                        board.save_max_storage(cx, window);
                    }))
                    .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                        cx.stop_propagation();
                    }),
            ),
        cx,
    )
}

fn render_autostart_row(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let toggle = Switch::new("autostart-toggle")
        .accessibility_label(t!("settings_autostart"))
        .checked(board.settings_editor.autostart.enabled)
        .on_click(cx.listener(|board, _, window, cx| {
            board.toggle_autostart(window, cx);
        }));
    settings_row(t!("settings_autostart"), toggle, cx)
}

fn render_confirm_mode_row(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let toggle = Switch::new("confirm-mode-toggle")
        .accessibility_label(t!("settings_paste_immediately"))
        .checked(matches!(
            board.confirm_mode,
            crate::config::ConfirmMode::PasteImmediately
        ))
        .on_click(cx.listener(|board, _, window, cx| {
            let new_mode = match board.confirm_mode {
                crate::config::ConfirmMode::CopyToClipboard => {
                    crate::config::ConfirmMode::PasteImmediately
                }
                crate::config::ConfirmMode::PasteImmediately => {
                    crate::config::ConfirmMode::CopyToClipboard
                }
            };
            board.save_confirm_mode(new_mode, window, cx);
        }));
    settings_row(t!("settings_paste_immediately"), toggle, cx)
}

fn render_hover_preview_row(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let toggle = Switch::new("hover-preview-toggle")
        .accessibility_label(t!("settings_hover_preview"))
        .checked(board.settings_editor.panel_state.hover_preview_enabled)
        .on_click(cx.listener(|board, _, window, cx| {
            board.save_hover_preview_enabled(
                !board.settings_editor.panel_state.hover_preview_enabled,
                window,
                cx,
            );
        }));
    settings_row(t!("settings_hover_preview"), toggle, cx)
}

fn render_space_preview_row(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let toggle = Switch::new("space-preview-toggle")
        .accessibility_label(t!("settings_space_preview"))
        .checked(board.settings_editor.panel_state.space_preview_enabled)
        .on_click(cx.listener(|board, _, window, cx| {
            board.save_space_preview_enabled(
                !board.settings_editor.panel_state.space_preview_enabled,
                window,
                cx,
            );
        }));
    settings_row(t!("settings_space_preview"), toggle, cx)
}

fn render_clear_history_row(cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    settings_row(
        t!("settings_clear_history"),
        h_flex()
            .items_center()
            .gap_2()
            .child(
                Button::new("clear-ordinary-history-button")
                    .small()
                    .ghost()
                    .border_1()
                    .border_color(cx.theme().border)
                    .label(t!("clear_ordinary"))
                    .tooltip(t!("clear_ordinary_tooltip"))
                    .on_click(cx.listener(|board, _, window, cx| {
                        board.open_clear_confirm(ClearConfirmAction::OrdinaryRecords, window, cx);
                    })),
            )
            .child(
                Button::new("clear-history-button")
                    .small()
                    .danger()
                    .label(t!("clear_all"))
                    .tooltip(t!("clear_all_tooltip"))
                    .on_click(cx.listener(|board, _, window, cx| {
                        board.open_clear_confirm(ClearConfirmAction::AllHistory, window, cx);
                    })),
            ),
        cx,
    )
}

fn render_open_dirs_row(cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let log_button = Button::new("open-log-button")
        .small()
        .ghost()
        .border_1()
        .border_color(cx.theme().border)
        .label(t!("settings_open_log"))
        .on_click(cx.listener(|_, _, _, _| {
            let log_dir = crate::utils::logging::log_dir();
            crate::utils::open_in_file_manager(&log_dir);
        }));

    let config_button = Button::new("open-config-button")
        .small()
        .ghost()
        .border_1()
        .border_color(cx.theme().border)
        .label(t!("settings_open_config"))
        .on_click(cx.listener(|_, _, _, _| {
            if let Ok(config_dir) = crate::config::Settings::config_dir() {
                crate::utils::open_in_file_manager(&config_dir);
            }
        }));

    h_flex()
        .justify_end()
        .items_center()
        .w_full()
        .py_3()
        .gap_1()
        .child(log_button)
        .child(config_button)
}

fn render_auto_check_row(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let toggle = Switch::new("auto-check-toggle")
        .accessibility_label(t!("update_auto_check"))
        .checked(board.settings_editor.update_settings.auto_check_enabled)
        .on_click(cx.listener(|board, _, window, cx| {
            board.save_auto_check_enabled(
                !board.settings_editor.update_settings.auto_check_enabled,
                window,
                cx,
            );
        }));
    settings_row(t!("update_auto_check"), toggle, cx)
}

fn render_include_prerelease_row(
    board: &RopyBoard,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let toggle = Switch::new("include-prerelease-toggle")
        .accessibility_label(t!("update_include_prerelease"))
        .checked(
            board
                .settings_editor
                .update_settings
                .include_prerelease_enabled,
        )
        .on_click(cx.listener(|board, _, window, cx| {
            board.save_include_prerelease_enabled(
                !board
                    .settings_editor
                    .update_settings
                    .include_prerelease_enabled,
                window,
                cx,
            );
        }));
    settings_row(t!("update_include_prerelease"), toggle, cx)
}

pub(crate) fn reset_settings_dialog(
    board: &mut RopyBoard,
    window: &mut gpui_kit::Window,
    cx: &mut Context<'_, RopyBoard>,
) {
    // Reset selections to persisted values from GPUI Global
    let settings = GlobalSettings::read(cx, Clone::clone);
    let lang_idx = crate::gui::settings::available_languages()
        .iter()
        .position(|language| language == &settings.language)
        .unwrap_or(0);
    let layout_idx = crate::config::LayoutMode::all()
        .iter()
        .position(|layout_mode| layout_mode == &settings.layout.mode)
        .unwrap_or_default();
    board.settings_editor.selected_language = lang_idx;
    board.settings_editor.selected_layout = layout_idx;
    board.settings_editor.autostart.enabled = settings.autostart.enabled;
    board.settings_editor.update_settings.auto_check_enabled = settings.update.auto_check;
    board
        .settings_editor
        .update_settings
        .include_prerelease_enabled = settings.update.include_prerelease;
    board.settings_editor.panel_state.hover_preview_enabled =
        settings.preview.hover_preview_enabled;
    board.settings_editor.panel_state.space_preview_enabled =
        settings.preview.space_preview_enabled;
    board.confirm_mode = settings.confirm.mode;
    board.layout_mode = settings.layout.mode;
    board.settings_editor.window_opacity_percent = settings.window.opacity_percent;
    board
        .settings_editor
        .pending_hotkey
        .clone_from(&settings.hotkey.activation_key);
    board.settings_editor.hotkey_before_recording = settings.hotkey.activation_key;
    board.settings_editor.hotkey.recording = false;

    // Reset the language select dropdown
    board
        .settings_editor
        .language_select
        .update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::default().row(lang_idx)), window, cx);
        });
    board.settings_editor.theme.sync(window, cx);
    board.sync_layout_select_items(window, cx);

    // Clear input fields
    board
        .settings_editor
        .settings_max_history_input
        .update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
    board
        .settings_editor
        .settings_max_storage_input
        .update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
    board
        .settings_editor
        .panel_state
        .window_opacity_slider_visible = false;
    board.sync_window_opacity_slider(settings.window.opacity_percent, window, cx);
    crate::gui::app::apply_current_theme(window, cx, settings.window.opacity_percent);
    crate::gui::app::apply_window_opacity(window, settings.window.opacity_percent);
    let current_hotkey = board.settings_editor.pending_hotkey.clone();
    board.sync_activation_key_input("", &current_hotkey, window, cx);

    board.active_panel = crate::gui::board::ActivePanel::ClipboardList;
    window.focus(&board.focus_handle, cx);
    cx.notify();
}

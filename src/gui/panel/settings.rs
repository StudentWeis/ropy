use gpui::{
    Context, div,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
    px,
};
use gpui_component::{
    ActiveTheme, IndexPath, Sizable,
    button::{Button, ButtonVariants},
    divider::Divider,
    h_flex,
    input::Input,
    scroll::ScrollableElement,
    select::Select,
    v_flex,
};

use crate::{gui::board::RopyBoard, i18n::Language, updater::models::UpdateStatus};

/// Render a settings row with label centered against the control.
fn settings_row<C: IntoElement>(
    label: impl Into<gpui::SharedString>,
    control: C,
    cx: &Context<RopyBoard>,
) -> impl IntoElement {
    h_flex()
        .justify_between()
        .items_center()
        .w_full()
        .py_3()
        .child(
            div()
                .flex_1()
                .text_sm()
                .text_color(cx.theme().foreground)
                .child(label.into()),
        )
        .child(control)
}

/// Render a settings row where the label is top-aligned (for multi-line right controls).
fn settings_row_top<C: IntoElement>(
    label: impl Into<gpui::SharedString>,
    control: C,
    cx: &Context<RopyBoard>,
) -> impl IntoElement {
    h_flex()
        .justify_between()
        .items_start()
        .w_full()
        .py_3()
        .child(
            div()
                .flex_1()
                .text_sm()
                .pt_1()
                .text_color(cx.theme().foreground)
                .child(label.into()),
        )
        .child(control)
}

/// Render the settings panel content — all items at the same level, left-right layout.
pub fn render_settings_content(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let header = render_settings_header(board, cx);
    let language_row = render_language_row(board, cx);
    let theme_row = render_theme_row(board, cx);
    let activation_key_row = render_activation_key_row(board, cx);
    let max_history_row = render_max_history_row(board, cx);
    let autostart_row = render_autostart_row(board, cx);
    let confirm_mode_row = render_confirm_mode_row(board, cx);
    let open_log_row = render_open_log_row(board, cx);
    let auto_check_row = render_auto_check_row(board, cx);
    let update_row = render_update_row(board, cx);
    let hover_preview_row = render_hover_preview_row(board, cx);

    v_flex().size_full().child(header).child(
        v_flex()
            .id("settings-content")
            .overflow_y_scrollbar()
            .flex_1()
            .px_4()
            .pb_4()
            .child(language_row)
            .child(Divider::horizontal())
            .child(theme_row)
            .child(Divider::horizontal())
            .child(activation_key_row)
            .child(Divider::horizontal())
            .child(max_history_row)
            .child(Divider::horizontal())
            .child(autostart_row)
            .child(Divider::horizontal())
            .child(confirm_mode_row)
            .child(Divider::horizontal())
            .child(hover_preview_row)
            .child(Divider::horizontal())
            .child(open_log_row)
            .child(Divider::horizontal())
            .child(auto_check_row)
            .child(Divider::horizontal())
            .child(update_row),
    )
}

fn render_settings_header(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let header = h_flex()
        .justify_between()
        .items_center()
        .mb_2()
        .px_4()
        .pt_4()
        .pb_3()
        .border_b_1()
        .border_color(cx.theme().border)
        .child(
            Button::new("cancel-button")
                .small()
                .ghost()
                .label(board.i18n.t("settings_cancel"))
                .on_click(cx.listener(|board, _click_event, window, cx| {
                    reset_settings_dialog(board, window, cx);
                }))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
        )
        .child(
            div()
                .text_lg()
                .text_color(cx.theme().foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child(board.i18n.t("settings_title")),
        )
        .child(
            Button::new("save-button")
                .small()
                .label(board.i18n.t("settings_save"))
                .on_click(cx.listener(|board, _, window, cx| {
                    board.save_settings(cx, window);
                }))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
        );

    #[cfg(target_os = "windows")]
    let header = header.on_mouse_down(gpui::MouseButton::Left, |_, window, _cx| {
        crate::gui::utils::start_window_drag(window);
    });

    header
}

fn render_language_row(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    settings_row(
        board.i18n.t("settings_language"),
        div()
            .flex_shrink_0()
            .child(Select::new(&board.language_select).small().w(px(140.0))),
        cx,
    )
}

fn render_theme_row(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let theme_names = [
        board.i18n.t("settings_theme_light"),
        board.i18n.t("settings_theme_dark"),
        board.i18n.t("settings_theme_system"),
    ];
    // Wrap in a border-grouped container to give a segmented-control look
    let theme_buttons = h_flex()
        .border_1()
        .border_color(cx.theme().border)
        .rounded_md()
        .overflow_hidden()
        .children(theme_names.into_iter().enumerate().map(|(index, name)| {
            let is_selected = board.selected_theme == index;
            let mut btn = Button::new(("theme-button", index)).small().label(name);
            btn = if is_selected {
                btn.primary()
            } else {
                btn.ghost()
            };
            btn.rounded_none()
                .on_click(cx.listener(move |board, _, _window, cx| {
                    board.selected_theme = index;
                    cx.notify();
                }))
        }));
    settings_row(board.i18n.t("settings_theme"), theme_buttons, cx)
}

fn render_activation_key_row(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    settings_row_top(
        board.i18n.t("settings_activation_key"),
        v_flex()
            .gap_1()
            .items_end()
            .child(
                Input::new(&board.settings_activation_key_input)
                    .appearance(false)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .w(px(180.0))
                    .px_3()
                    .py_1(),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(board.i18n.t("settings_hotkey_hint")),
            ),
        cx,
    )
}

fn render_max_history_row(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    settings_row(
        board.i18n.t("settings_max_history"),
        Input::new(&board.settings_max_history_input)
            .appearance(false)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .w(px(70.0))
            .px_3()
            .py_1(),
        cx,
    )
}

fn render_autostart_row(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let mut btn = Button::new("autostart-toggle").small();
    btn = if board.autostart_enabled {
        btn.primary().label(board.i18n.t("on"))
    } else {
        btn.ghost().label(board.i18n.t("off"))
    };
    let toggle = btn.on_click(cx.listener(|board, _, _, cx| board.toggle_autostart(cx)));
    settings_row(board.i18n.t("settings_autostart"), toggle, cx)
}

fn render_confirm_mode_row(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let mode_names = [
        board.i18n.t("settings_confirm_mode_copy"),
        board.i18n.t("settings_confirm_mode_paste"),
    ];
    let mode_buttons = h_flex()
        .border_1()
        .border_color(cx.theme().border)
        .rounded_md()
        .overflow_hidden()
        .children(mode_names.into_iter().enumerate().map(|(index, name)| {
            let is_selected = matches!(
                (index, board.confirm_mode),
                (0, crate::config::ConfirmMode::CopyToClipboard)
                    | (1, crate::config::ConfirmMode::PasteImmediately)
            );
            let mut btn = Button::new(("confirm-mode-button", index))
                .small()
                .label(name);
            btn = if is_selected {
                btn.primary()
            } else {
                btn.ghost()
            };
            btn.rounded_none()
                .on_click(cx.listener(move |board, _, window, cx| {
                    let mode = if index == 0 {
                        crate::config::ConfirmMode::CopyToClipboard
                    } else {
                        crate::config::ConfirmMode::PasteImmediately
                    };
                    board.set_confirm_mode(mode, window);
                    cx.notify();
                }))
        }));

    settings_row(board.i18n.t("settings_confirm_mode"), mode_buttons, cx)
}

fn render_hover_preview_row(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let mut btn = Button::new("hover-preview-toggle").small();
    btn = if board.hover_preview_enabled {
        btn.primary().label(board.i18n.t("on"))
    } else {
        btn.ghost().label(board.i18n.t("off"))
    };
    let toggle = btn.on_click(cx.listener(|board, _, _, cx| {
        board.hover_preview_enabled = !board.hover_preview_enabled;
        cx.notify();
    }));
    settings_row(board.i18n.t("settings_hover_preview"), toggle, cx)
}

fn render_open_log_row(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let btn = Button::new("open-log-button")
        .small()
        .ghost()
        .label(board.i18n.t("settings_open_log"))
        .on_click(cx.listener(|_, _, _, _| {
            let log_dir = crate::logging::log_dir();
            #[cfg(target_os = "macos")]
            let _ = std::process::Command::new("open").arg(&log_dir).spawn();
            #[cfg(target_os = "windows")]
            let _ = std::process::Command::new("explorer").arg(&log_dir).spawn();
            #[cfg(target_os = "linux")]
            let _ = std::process::Command::new("xdg-open").arg(&log_dir).spawn();
        }));
    settings_row(board.i18n.t("settings_open_log"), btn, cx)
}

fn render_auto_check_row(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let mut btn = Button::new("auto-check-toggle").small();
    btn = if board.auto_check_enabled {
        btn.primary().label(board.i18n.t("on"))
    } else {
        btn.ghost().label(board.i18n.t("off"))
    };
    let toggle = btn.on_click(cx.listener(|board, _, _, cx| {
        board.auto_check_enabled = !board.auto_check_enabled;
        cx.notify();
    }));
    settings_row(board.i18n.t("update_auto_check"), toggle, cx)
}

fn render_update_row(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let version = crate::updater::checker::current_version();
    let status_text: gpui::SharedString = match &board.update_status {
        UpdateStatus::Idle => format!("v{version}").into(),
        UpdateStatus::Checking => board.i18n.t("update_checking").into(),
        UpdateStatus::Available(info) => {
            format!("{}: v{}", board.i18n.t("update_available"), info.version).into()
        }
        UpdateStatus::UpToDate => board.i18n.t("update_up_to_date").into(),
        UpdateStatus::Downloading(p) => {
            format!("{} {:.0}%", board.i18n.t("update_downloading"), p * 100.0).into()
        }
        UpdateStatus::ReadyToRestart => board.i18n.t("update_restart").into(),
        UpdateStatus::Error(msg) => format!("{}: {}", board.i18n.t("update_error"), msg).into(),
    };
    let status_color = match &board.update_status {
        UpdateStatus::Available(_) | UpdateStatus::ReadyToRestart => cx.theme().foreground,
        UpdateStatus::Error(_) => gpui::rgb(0x00cc_3333).into(),
        _ => cx.theme().muted_foreground,
    };

    let action_button: Option<Button> = match &board.update_status {
        UpdateStatus::Available(_) => Some(
            Button::new("update-download-button")
                .small()
                .primary()
                .label(board.i18n.t("update_download"))
                .on_click(cx.listener(|board, _, _, cx| board.download_and_install_update(cx))),
        ),
        UpdateStatus::ReadyToRestart => Some(
            Button::new("update-restart-button")
                .small()
                .primary()
                .label(board.i18n.t("update_restart_button"))
                .on_click(cx.listener(|_, _, _, cx| {
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::process::Command::new(exe).spawn();
                    }
                    cx.quit();
                })),
        ),
        UpdateStatus::Idle | UpdateStatus::UpToDate | UpdateStatus::Error(_) => Some(
            Button::new("update-check-button")
                .small()
                .ghost()
                .label(board.i18n.t("update_check_now"))
                .on_click(cx.listener(|board, _, _, cx| board.check_for_update_async(cx))),
        ),
        _ => None,
    };

    let mut right_col = v_flex().gap_1().items_end();
    right_col = right_col.child(div().text_xs().text_color(status_color).child(status_text));
    if let Some(btn) = action_button {
        right_col = right_col.child(btn);
    }
    settings_row_top(board.i18n.t("update_title"), right_col, cx)
}

pub fn reset_settings_dialog(
    board: &mut RopyBoard,
    window: &mut gpui::Window,
    cx: &mut Context<'_, RopyBoard>,
) {
    // Reset selections to persisted values
    let settings_guard = match board.settings.read() {
        Ok(g) => g,
        Err(e) => e.into_inner(),
    };
    let lang_idx = Language::all()
        .iter()
        .position(|lang| lang == &settings_guard.language)
        .unwrap_or(0);
    board.selected_language = lang_idx;
    board.selected_theme = match settings_guard.theme {
        crate::config::AppTheme::Light => 0,
        crate::config::AppTheme::Dark => 1,
        crate::config::AppTheme::System => 2,
    };
    board.autostart_enabled = settings_guard.autostart.enabled;
    board.auto_check_enabled = settings_guard.update.auto_check;
    board.confirm_mode = settings_guard.confirm.mode;
    drop(settings_guard);

    // Reset the language select dropdown
    board.language_select.update(cx, |state, cx| {
        state.set_selected_index(Some(IndexPath::default().row(lang_idx)), window, cx);
    });

    // Clear input fields
    board.settings_max_history_input.update(cx, |input, cx| {
        input.set_value("", window, cx);
    });
    board.settings_activation_key_input.update(cx, |input, cx| {
        input.set_value("", window, cx);
    });

    board.show_settings = false;
    window.focus(&board.focus_handle);
    cx.notify();
}

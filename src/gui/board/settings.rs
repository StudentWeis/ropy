use gpui::{
    Context, div,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
    px,
};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
    scroll::ScrollableElement,
    v_flex,
};

use super::RopyBoard;
use crate::{
    i18n::{I18n, Language},
    updater::models::UpdateStatus,
};

/// Render language selection buttons
/// Note: Uses index-based selection from `Language::all()` which maintains a stable order.
/// The order is: English, `ChineseSimplified`
fn render_language_selector(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let languages = Language::all();

    h_flex()
        .gap_2()
        .items_center()
        .children(languages.iter().enumerate().map(|(index, lang)| {
            let is_selected = board.selected_language == index;
            let lang_copy = *lang; // Copy the language for the closure

            let mut button = Button::new(("language-button", index))
                .small()
                .label(lang.display_name());

            button = if is_selected {
                button.primary()
            } else {
                button.ghost()
            };

            button.on_click(cx.listener(move |board, _, window, cx| {
                board.selected_language = index;
                // Update search placeholder immediately for instant feedback
                if let Ok(temp_i18n) = I18n::new(lang_copy) {
                    board.search_input.update(cx, |input, cx| {
                        input.set_placeholder(temp_i18n.t("search_placeholder"), window, cx);
                    });
                }
                cx.notify();
            }))
        }))
}

/// Render theme selection buttons
fn render_theme_selector(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let theme_names = vec![
        board.i18n.t("settings_theme_light"),
        board.i18n.t("settings_theme_dark"),
        board.i18n.t("settings_theme_system"),
    ];

    h_flex()
        .gap_2()
        .items_center()
        .children(theme_names.into_iter().enumerate().map(|(index, name)| {
            let is_selected = board.selected_theme == index;
            let mut button = Button::new(("theme-button", index)).small().label(name);
            button = if is_selected {
                button.primary()
            } else {
                button.ghost()
            };
            button.on_click(cx.listener(move |board, _, _window, cx| {
                board.selected_theme = index;
                cx.notify();
            }))
        }))
}

fn render_storage_section(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let max_history_input_field = h_flex()
        .gap_2()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_color(cx.theme().foreground)
                .child(board.i18n.t("settings_max_history")),
        )
        .child(
            Input::new(&board.settings_max_history_input)
                .appearance(false)
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .w(px(60.0))
                .px_3()
                .py_2(),
        );

    v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child(board.i18n.t("settings_storage")),
        )
        .child(max_history_input_field)
}

fn render_hotkey_section(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let activation_key_label = v_flex()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().foreground)
                .child(board.i18n.t("settings_activation_key")),
        )
        .child(
            Input::new(&board.settings_activation_key_input)
                .appearance(false)
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .px_3()
                .py_2(),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(board.i18n.t("settings_hotkey_hint")),
        );

    v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child(board.i18n.t("settings_hotkey")),
        )
        .child(activation_key_label)
}

fn render_system_section(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child(board.i18n.t("settings_system")),
        )
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(
                    div()
                        .text_color(cx.theme().foreground)
                        .child(board.i18n.t("settings_autostart")),
                )
                .child({
                    let mut button = Button::new("autostart-toggle").small();

                    button = if board.autostart_enabled {
                        button
                            .primary()
                            .label(board.i18n.t("settings_autostart_on"))
                    } else {
                        button.ghost().label(board.i18n.t("settings_autostart_off"))
                    };

                    button.on_click(cx.listener(|board, _, _, cx| {
                        board.toggle_autostart(cx);
                    }))
                }),
        )
}

fn render_update_status_element(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let version = crate::updater::checker::current_version();

    match &board.update_status {
        UpdateStatus::Idle => div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(format!(
                "{}: v{}",
                board.i18n.t("update_current_version"),
                version
            )),
        UpdateStatus::Checking => div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(board.i18n.t("update_checking")),
        UpdateStatus::Available(info) => {
            div()
                .text_xs()
                .text_color(cx.theme().foreground)
                .child(format!(
                    "{}: v{}",
                    board.i18n.t("update_available"),
                    info.version
                ))
        }
        UpdateStatus::UpToDate => div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(board.i18n.t("update_up_to_date")),
        UpdateStatus::Downloading(progress) => div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(format!(
                "{} {:.0}%",
                board.i18n.t("update_downloading"),
                progress * 100.0
            )),
        UpdateStatus::ReadyToRestart => div()
            .text_xs()
            .text_color(cx.theme().foreground)
            .child(board.i18n.t("update_restart")),
        UpdateStatus::Error(msg) => div()
            .text_xs()
            .text_color(gpui::rgb(0x00cc_3333))
            .child(format!("{}: {}", board.i18n.t("update_error"), msg)),
    }
}

fn render_update_action_button(board: &RopyBoard, cx: &Context<RopyBoard>) -> Option<Button> {
    match &board.update_status {
        UpdateStatus::Available(_) => Some(
            Button::new("update-download-button")
                .small()
                .primary()
                .label(board.i18n.t("update_download"))
                .on_click(cx.listener(|board, _, _, cx| {
                    board.download_and_install_update(cx);
                })),
        ),
        UpdateStatus::ReadyToRestart => Some(
            Button::new("update-restart-button")
                .small()
                .primary()
                .label(board.i18n.t("update_restart_button"))
                .on_click(cx.listener(|_, _, _, cx| {
                    // Restart the application
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
                .on_click(cx.listener(|board, _, _, cx| {
                    board.check_for_update_async(cx);
                })),
        ),
        _ => None, // Checking or Downloading — no button
    }
}

fn render_auto_check_row(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    h_flex()
        .justify_between()
        .items_center()
        .child(
            div()
                .text_color(cx.theme().foreground)
                .child(board.i18n.t("update_auto_check")),
        )
        .child({
            let mut button = Button::new("auto-check-toggle").small();
            button = if board.auto_check_enabled {
                button.primary().label(board.i18n.t("update_on"))
            } else {
                button.ghost().label(board.i18n.t("update_off"))
            };
            button.on_click(cx.listener(|board, _, _, cx| {
                board.auto_check_enabled = !board.auto_check_enabled;
                cx.notify();
            }))
        })
}

fn render_update_section(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let status_element = render_update_status_element(board, cx);
    let action_button = render_update_action_button(board, cx);
    let auto_check_row = render_auto_check_row(board, cx);

    let mut section = v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child(board.i18n.t("update_title")),
        )
        .child(auto_check_row)
        .child(status_element);

    if let Some(btn) = action_button {
        section = section.child(btn);
    }

    section
}

/// Render the settings panel content
pub(super) fn render_settings_content(
    board: &RopyBoard,
    cx: &Context<RopyBoard>,
) -> impl IntoElement {
    let language_section = v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child(board.i18n.t("settings_language")),
        )
        .child(render_language_selector(board, cx));

    let theme_section = v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child(board.i18n.t("settings_theme")),
        )
        .child(render_theme_selector(board, cx));

    let header = h_flex()
        .justify_between()
        .items_center()
        .mb_4()
        .pt_4()
        .child(
            Button::new("cancel-button")
                .small()
                .ghost()
                .label(board.i18n.t("settings_cancel"))
                .on_click(cx.listener(|board, _click_event, window, cx| {
                    reset_settings_dialog(board, window, cx);
                })),
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
                })),
        );

    #[cfg(target_os = "windows")]
    let header = header.on_mouse_down(gpui::MouseButton::Left, |_, window, _cx| {
        crate::gui::utils::start_window_drag(window);
    });

    v_flex().size_full().child(header).child(
        v_flex()
            .id("settings-content")
            .overflow_y_scrollbar()
            .gap_4()
            .flex_1()
            .child(language_section)
            .child(theme_section)
            .child(render_hotkey_section(board, cx))
            .child(render_storage_section(board, cx))
            .child(render_system_section(board, cx))
            .child(render_update_section(board, cx)),
    )
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
    board.selected_language = Language::all()
        .iter()
        .position(|&lang| lang == settings_guard.language)
        .unwrap_or(0);
    board.selected_theme = match settings_guard.theme {
        crate::config::AppTheme::Light => 0,
        crate::config::AppTheme::Dark => 1,
        crate::config::AppTheme::System => 2,
    };
    board.autostart_enabled = settings_guard.autostart.enabled;
    board.auto_check_enabled = settings_guard.update.auto_check;
    drop(settings_guard);

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

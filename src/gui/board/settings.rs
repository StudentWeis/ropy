use crate::i18n::{I18n, Language};
use gpui::{
    Context, div,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
    px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::{ActiveTheme, Sizable, h_flex, v_flex};

use super::RopyBoard;
#[cfg(target_os = "windows")]
use crate::gui::utils::start_window_drag;

/// Render language selection buttons
/// Note: Uses index-based selection from Language::all() which maintains a stable order.
/// The order is: English, ChineseSimplified
fn render_language_selector(
    board: &mut RopyBoard,
    cx: &mut Context<RopyBoard>,
) -> impl IntoElement {
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
fn render_theme_selector(board: &mut RopyBoard, cx: &mut Context<RopyBoard>) -> impl IntoElement {
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

/// Render the settings panel content
pub(super) fn render_settings_content(
    board: &mut RopyBoard,
    cx: &mut Context<RopyBoard>,
) -> impl IntoElement {
    let setting_button_group = h_flex()
        .gap_2()
        .justify_end()
        .mt_4()
        .child(
            Button::new("cancel-button")
                .small()
                .ghost()
                .label(board.i18n.t("settings_cancel"))
                .on_click(cx.listener(|board, _, window, cx| {
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
                })),
        )
        .child(
            Button::new("save-button")
                .small()
                .label(board.i18n.t("settings_save"))
                .on_click(cx.listener(|board, _, window, cx| {
                    board.save_settings(cx, window);
                })),
        );
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
    let hotkey_section = v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child(board.i18n.t("settings_hotkey")),
        )
        .child(activation_key_label);

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
    let storage_section = v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child(board.i18n.t("settings_storage")),
        )
        .child(max_history_input_field);
    let autostart_section = v_flex()
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
        );
    let header = h_flex()
        .justify_between()
        .items_center()
        .mb_4()
        .pt_4()
        .child(
            Button::new("back-button")
                .small()
                .ghost()
                .label(board.i18n.t("settings_back"))
                .on_click(cx.listener(|board, _, window, cx| {
                    board.show_settings = false;
                    window.focus(&board.focus_handle);
                    cx.notify();
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
        .child(div().w(px(55.)));

    #[cfg(target_os = "windows")]
    let header = header.on_mouse_down(gpui::MouseButton::Left, |_, window, cx| {
        start_window_drag(window, cx);
    });

    v_flex()
        .size_full()
        .child(header)
        .child(
            v_flex()
                .gap_4()
                .flex_1()
                .child(language_section)
                .child(theme_section)
                .child(hotkey_section)
                .child(storage_section)
                .child(autostart_section),
        )
        .child(setting_button_group)
}

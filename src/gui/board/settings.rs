use gpui::{
    Context, div,
    prelude::{IntoElement, ParentElement, Styled},
    px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::Input;
use gpui_component::{ActiveTheme, Sizable, h_flex, v_flex};

use super::RopyBoard;

/// Render theme selection buttons
fn render_theme_selector(board: &mut RopyBoard, cx: &mut Context<RopyBoard>) -> impl IntoElement {
    let themes = [("Light", 0), ("Dark", 1), ("System", 2)];

    h_flex()
        .gap_2()
        .items_center()
        .children(themes.iter().map(|(name, index)| {
            let is_selected = board.selected_theme == *index;
            let index_val = *index;

            let mut button = Button::new(("theme-button", index_val))
                .small()
                .label(*name);

            button = if is_selected {
                button.primary()
            } else {
                button.ghost()
            };

            button.on_click(cx.listener(move |board, _, _window, cx| {
                board.selected_theme = index_val;
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
                .label("Cancel")
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
                .label("Save")
                .on_click(cx.listener(|board, _, window, cx| {
                    board.save_settings(cx, window);
                })),
        );
    let max_history_input_field = v_flex()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().foreground)
                .child("Max History Records"),
        )
        .child(
            Input::new(&board.settings_max_history_input)
                .appearance(false)
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .px_3()
                .py_2(),
        );
    let activation_key_label = v_flex()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().foreground)
                .child("Activation Key"),
        )
        .child(
            Input::new(&board.settings_activation_key_input)
                .appearance(false)
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .px_3()
                .py_2(),
        );
    #[allow(unused_variables)]
    let hotkey_section = v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child("Hotkey Configuration"),
        )
        .child(activation_key_label);
    let theme_section = v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child("Theme"),
        )
        .child(render_theme_selector(board, cx));
    let storage_section = v_flex()
        .gap_2()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child("Storage Configuration"),
        )
        .child(max_history_input_field);
    v_flex()
        .size_full()
        .child(
            // Header with back button
            h_flex()
                .justify_between()
                .items_center()
                .mb_4()
                .child(
                    Button::new("back-button")
                        .small()
                        .ghost()
                        .label("←")
                        .on_click(cx.listener(|board, _, window, cx| {
                            board.show_settings = false;
                            window.focus(&board.focus_handle);
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .text_lg()
                        .text_color(cx.theme().foreground)
                        .font_weight(gpui::FontWeight::BOLD)
                        .child("Ropy Settings"),
                )
                .child(div().w(px(55.))), // Spacer for centering
        )
        .child(
            v_flex()
                .gap_4()
                .flex_1()
                .child(theme_section)
                // .child(hotkey_section)
                .child(storage_section),
        )
        .child(setting_button_group)
}

use gpui::{
    Context,
    prelude::{FluentBuilder, InteractiveElement, IntoElement, ParentElement, Styled},
};
use gpui_component::{
    ActiveTheme, Icon,
    button::{Button, ButtonVariants},
    h_flex,
};

use super::RopyBoard;
use crate::{constants::APP_NAME, i18n::I18n};

pub fn render_header(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let is_pinned = board.pinned;
    let can_toggle_window_pin = board.can_toggle_window_pin();
    let pin_tooltip = if !can_toggle_window_pin {
        I18n::translate(cx, "pin_unavailable")
    } else if is_pinned {
        I18n::translate(cx, "unpin")
    } else {
        I18n::translate(cx, "pin")
    };
    let header = h_flex().justify_between().items_center().mb_4().pt_4();

    #[cfg(target_os = "windows")]
    let header = header.on_mouse_down(gpui::MouseButton::Left, |_, window, _cx| {
        crate::gui::utils::start_window_drag(window);
    });

    header
        .child(
            h_flex().items_center().gap_2().child(
                gpui::div()
                    .text_lg()
                    .text_color(cx.theme().foreground)
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(APP_NAME),
            ),
        )
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .child(
                    if is_pinned {
                        Button::new("pin-button").primary()
                    } else {
                        Button::new("pin-button").ghost().opacity(0.5_f32)
                    }
                    .when(can_toggle_window_pin, |button| button.opacity(1.0_f32))
                    .icon(Icon::empty().path("icons/pin-to-top.svg"))
                    .tooltip(pin_tooltip)
                    .on_click(
                        cx.listener(|this, _event, #[allow(unused_variables)] window, cx| {
                            this.toggle_window_pin(window);
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(|_, _, _, cx| cx.stop_propagation()),
                    ),
                )
                .child(
                    Button::new("help-button")
                        .ghost()
                        .icon(Icon::empty().path("icons/help.svg"))
                        .tooltip(I18n::translate(cx, "help_title"))
                        .on_click(cx.listener(|this, _click_event, _window, cx| {
                            this.active_panel = crate::gui::board::ActivePanel::Help;
                            cx.notify();
                        }))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                )
                .child(
                    Button::new("about-button")
                        .ghost()
                        .icon(Icon::empty().path("icons/info.svg"))
                        .tooltip(I18n::translate(cx, "about_title"))
                        .on_click(cx.listener(|this, _click_event, _window, cx| {
                            this.active_panel = crate::gui::board::ActivePanel::About;
                            cx.notify();
                        }))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                )
                .child(
                    Button::new("settings-button")
                        .ghost()
                        .icon(Icon::empty().path("icons/settings.svg"))
                        .tooltip(I18n::translate(cx, "settings_button"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_settings_panel(window, cx);
                            cx.notify();
                        }))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                ),
        )
}

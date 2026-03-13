use gpui::{
    Context, div,
    prelude::{
        FluentBuilder, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
        Styled,
    },
    px,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::Input,
    v_flex,
};

use super::RopyBoard;
use crate::constants::APP_NAME;

/// Create the "Clear" button element
pub(super) fn create_clear_button(
    board: &RopyBoard,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    Button::new("clear-button")
        .ghost()
        .icon(Icon::empty().path("icon/clear-all.svg"))
        .tooltip(board.i18n.t("clear_all"))
        .on_click(cx.listener(|this, _, _, cx| {
            this.show_clear_confirm = true;
            cx.notify();
        }))
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

/// Render the clear-all confirmation overlay (backdrop + centered dialog card)
pub(super) fn render_clear_confirm_overlay(
    board: &RopyBoard,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let title = board.i18n.t("clear_confirm_title");
    let message = board.i18n.t("clear_confirm_message");
    let cancel_label = board.i18n.t("clear_confirm_cancel");
    let confirm_label = board.i18n.t("clear_confirm_button");

    div()
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .bg(gpui::rgba(0x0000_0050))
        .flex()
        .items_center()
        .justify_center()
        .id("clear-confirm-backdrop")
        .on_click(cx.listener(|this, _, _, cx| {
            this.show_clear_confirm = false;
            cx.notify();
        }))
        .child(
            v_flex()
                .w(px(300.0))
                .p_5()
                .bg(cx.theme().background)
                .border_1()
                .border_color(cx.theme().border)
                .rounded_lg()
                .gap_3()
                .id("clear-confirm-card")
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .text_base()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(cx.theme().foreground)
                        .child(title),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(message),
                )
                .child(
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("clear-confirm-cancel")
                                .small()
                                .ghost()
                                .label(cancel_label)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.show_clear_confirm = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("clear-confirm-ok")
                                .small()
                                .danger()
                                .label(confirm_label)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.clear_history();
                                    this.clear_last_copy_state();
                                    this.show_clear_confirm = false;
                                    cx.notify();
                                })),
                        ),
                ),
        )
}

/// Render the header section with title and settings/clear buttons
pub fn render_header(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let is_pinned = board.pinned;
    let show_pin_button = board.can_toggle_window_pin();
    let pin_tooltip = if is_pinned {
        board.i18n.t("unpin")
    } else {
        board.i18n.t("pin")
    };
    let header = h_flex().justify_between().items_center().mb_4().pt_4();

    #[cfg(target_os = "windows")]
    let header = header.on_mouse_down(gpui::MouseButton::Left, |_, window, _cx| {
        crate::gui::utils::start_window_drag(window);
    });

    header
        .child(
            div()
                .text_lg()
                .text_color(cx.theme().foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child(APP_NAME),
        )
        .child(
            h_flex()
                .gap_2()
                .items_center()
                .when(show_pin_button, |el| {
                    el.child(
                        if is_pinned {
                            Button::new("pin-button").primary()
                        } else {
                            Button::new("pin-button").ghost()
                        }
                        .icon(Icon::empty().path("icon/pin-to-top.svg"))
                        .tooltip(pin_tooltip)
                        .on_click(cx.listener(
                            |this, _event, #[allow(unused_variables)] window, cx| {
                                this.toggle_window_pin(window);
                                cx.notify();
                            },
                        ))
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|_, _, _, cx| cx.stop_propagation()),
                        ),
                    )
                })
                .child(
                    Button::new("help-button")
                        .ghost()
                        .icon(Icon::empty().path("icon/help.svg"))
                        .tooltip(board.i18n.t("help_title"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.show_help = true;
                            window.focus(&this.focus_handle);
                            cx.notify();
                        }))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                )
                .child(
                    Button::new("about-button")
                        .ghost()
                        .icon(Icon::empty().path("icon/info.svg"))
                        .tooltip(board.i18n.t("about_title"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.show_about = true;
                            window.focus(&this.focus_handle);
                            cx.notify();
                        }))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                )
                .child(
                    Button::new("settings-button")
                        .ghost()
                        .icon(Icon::empty().path("icon/settings.svg"))
                        .tooltip(board.i18n.t("settings_button"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.show_settings = true;
                            window.focus(&this.focus_handle);
                            cx.notify();
                        }))
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
                )
                .child(create_clear_button(board, cx)),
        )
}

/// Render the search input section with content type filter buttons
pub(super) fn render_search_input(
    board: &RopyBoard,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    use super::ContentFilter;

    let is_text_active = board.content_filter == ContentFilter::Text;
    let is_image_active = board.content_filter == ContentFilter::Image;

    let text_filter_tooltip = board.i18n.t("filter_text");
    let image_filter_tooltip = board.i18n.t("filter_image");

    let text_button = if is_text_active {
        Button::new("filter-text-btn").primary()
    } else {
        Button::new("filter-text-btn").ghost()
    };

    let image_button = if is_image_active {
        Button::new("filter-image-btn").primary()
    } else {
        Button::new("filter-image-btn").ghost()
    };

    h_flex()
        .w_full()
        .mb_4()
        .gap_2()
        .child(
            div().flex_1().min_w_0().child(
                Input::new(&board.search_input)
                    .appearance(false)
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded_md()
                    .px_3()
                    .py_2(),
            ),
        )
        .child(
            text_button
                .icon(Icon::empty().path("icon/filter-text.svg"))
                .tooltip(text_filter_tooltip)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_content_filter(ContentFilter::Text);
                    cx.notify();
                }))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
        )
        .child(
            image_button
                .icon(Icon::empty().path("icon/filter-image.svg"))
                .tooltip(image_filter_tooltip)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.toggle_content_filter(ContentFilter::Image);
                    cx.notify();
                }))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
        )
}

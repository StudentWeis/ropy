use gpui::{
    Context, div,
    prelude::{InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled},
    px,
};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use super::RopyBoard;

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

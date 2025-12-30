use gpui::{
    Context, ImageSource, Resource, div, img,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
    px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{ActiveTheme, Sizable, h_flex, v_flex};

use super::RopyBoard;
#[cfg(target_os = "windows")]
use crate::gui::utils::start_window_drag;

/// Render the about panel content
pub(super) fn render_about_content(
    board: &mut RopyBoard,
    cx: &mut Context<RopyBoard>,
) -> impl IntoElement {
    let version = env!("CARGO_PKG_VERSION");

    let header = h_flex()
        .justify_between()
        .items_center()
        .mb_4()
        .pt_4()
        .child(
            Button::new("back-button")
                .small()
                .ghost()
                .label(board.i18n.t("about_back"))
                .on_click(cx.listener(|board, _, window, cx| {
                    board.show_about = false;
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
                .child(board.i18n.t("about_title")),
        )
        .child(div().w(px(55.)));

    #[cfg(target_os = "windows")]
    let header = header.on_mouse_down(gpui::MouseButton::Left, |_, window, cx| {
        start_window_drag(window, cx);
    });

    v_flex().size_full().child(header).child(
        v_flex()
            .flex_1()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                img(ImageSource::Resource(Resource::Embedded("logo.png".into())))
                    .w(px(100.0))
                    .h(px(100.0))
                    .rounded_md(),
            )
            .child(
                div()
                    .text_2xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(cx.theme().foreground)
                    .child(board.i18n.t("app_name")),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{} {}", board.i18n.t("about_version"), version)),
            )
            .child(
                div()
                    .px_8()
                    .text_center()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(board.i18n.t("about_description")),
            )
            .child(
                Button::new("github-button")
                    .ghost()
                    .label(board.i18n.t("about_github"))
                    .on_click(|_, _, cx| {
                        cx.open_url("https://github.com/StudentWeis/ropy");
                    }),
            ),
    )
}

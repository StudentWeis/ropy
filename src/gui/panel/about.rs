use gpui::{
    Context, ImageSource, Resource, StatefulInteractiveElement, div, img,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
    px,
};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use crate::{constants::ABOUT_BACK_ARROW, gui::board::RopyBoard, updater::models::UpdateStatus};

/// Render the about panel content
pub fn render_about_content(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
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
                .label(ABOUT_BACK_ARROW)
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
    let header = header.on_mouse_down(gpui::MouseButton::Left, |_, window, _cx| {
        crate::gui::utils::start_window_drag(window);
    });

    v_flex().size_full().child(header).child(
        v_flex()
            .id("about-content")
            .overflow_y_scroll()
            .flex_1()
            .items_center()
            .gap_3()
            // Logo
            .child(
                img(ImageSource::Resource(Resource::Embedded("logo.png".into())))
                    .w(px(80.0))
                    .h(px(80.0))
                    .rounded_md(),
            )
            // Version text
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .child(format!("{} {}", board.i18n.t("about_version"), version)),
            )
            // Update section
            .child(render_update_section(board, cx))
            // Description and GitHub footer
            .child(
                div()
                    .px_8()
                    .text_center()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(board.i18n.t("about_description")),
            )
            .child(
                Button::new("github-button")
                    .ghost()
                    .label("GitHub")
                    .on_click(|_, _, cx| {
                        cx.open_url("https://github.com/StudentWeis/ropy");
                    }),
            ),
    )
}

/// Render the update section with status and action button.
fn render_update_section(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let status_text: gpui::SharedString = match &board.update_status {
        UpdateStatus::Idle => board.i18n.t("update_check_now").into(),
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

    h_flex()
        .items_center()
        .gap_2()
        .child(div().text_xs().text_color(status_color).child(status_text))
        .children(action_button)
}

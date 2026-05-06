use gpui::{
    Context, ImageSource, Resource, StatefulInteractiveElement, div, img,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
    px,
};
use gpui_component::{
    ActiveTheme, Icon, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use crate::{
    gui::{
        board::RopyBoard,
        panel::common::{panel_back_button, panel_header_with_back},
    },
    i18n::I18n,
    updater::models::UpdateStatus,
};

const LICENSE_URL: &str = "https://github.com/StudentWeis/ropy/blob/main/LICENSE";
const GITHUB_URL: &str = "https://github.com/StudentWeis/ropy";
const XIAOHONGSHU_URL: &str = "https://xhslink.com/m/Ar6O3JkYc0X";

fn render_social_link_button(
    id: &'static str,
    icon: Icon,
    tooltip: String,
    url: &'static str,
) -> impl IntoElement {
    div().rounded_md().p_1().child(
        Button::new(id)
            .ghost()
            .icon(icon)
            .tooltip(tooltip)
            .on_click(move |_, _, cx| {
                cx.open_url(url);
            }),
    )
}

pub fn render_about_content(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let version = env!("CARGO_PKG_VERSION");

    let header = panel_header_with_back(
        I18n::translate(cx, "about_title"),
        panel_back_button("back-button")
            .on_click(cx.listener(|board, _, window, cx| {
                board.active_panel = crate::gui::board::ActivePanel::ClipboardList;
                window.focus(&board.focus_handle);
                cx.notify();
            }))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
        cx,
    );

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
                    .child(format!(
                        "{} {}",
                        I18n::translate(cx, "about_version"),
                        version
                    )),
            )
            // Update section
            .child(render_update_section(board, cx))
            // Description
            .child(
                div()
                    .px_8()
                    .text_center()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(I18n::translate(cx, "about_description")),
            )
            // License
            .child(
                Button::new("license-button")
                    .small()
                    .ghost()
                    .label(I18n::translate(cx, "about_license"))
                    .on_click(|_, _, cx| {
                        cx.open_url(LICENSE_URL);
                    }),
            )
            // Social links with icons
            .child(
                h_flex()
                    .gap_2()
                    .child(render_social_link_button(
                        "github-button",
                        Icon::empty().path("icons/github.svg").size(px(32.0)),
                        I18n::translate(cx, "about_github_tooltip"),
                        GITHUB_URL,
                    ))
                    .child(render_social_link_button(
                        "xiaohongshu-button",
                        Icon::empty().path("icons/xiaohongshu.svg").size(px(32.0)),
                        I18n::translate(cx, "about_xiaohongshu_tooltip"),
                        XIAOHONGSHU_URL,
                    )),
            ),
    )
}

fn render_update_section(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let status_text: gpui::SharedString = match &board.update_manager.status {
        UpdateStatus::Idle => I18n::translate(cx, "update_check_now").into(),
        UpdateStatus::Checking => I18n::translate(cx, "update_checking").into(),
        UpdateStatus::Available(info) => format!(
            "{}: v{}",
            I18n::translate(cx, "update_available"),
            info.version
        )
        .into(),
        UpdateStatus::UpToDate => I18n::translate(cx, "update_up_to_date").into(),
        UpdateStatus::Downloading(p) => format!(
            "{} {:.0}%",
            I18n::translate(cx, "update_downloading"),
            p * 100.0
        )
        .into(),
        UpdateStatus::ReadyToRestart => I18n::translate(cx, "update_restart").into(),
        UpdateStatus::Error(msg) => {
            // Surface a localized "network problem" hint for the most common
            // class of update failures so users aren't shown raw curl /
            // TLS errors.
            let friendly_msg = if msg.contains("curl")
                || msg.contains("SSL")
                || msg.contains("HTTP request failed")
            {
                I18n::translate(cx, "update_error_network")
            } else {
                I18n::translate(cx, "update_error")
            };
            friendly_msg.into()
        }
    };
    let status_color = match &board.update_manager.status {
        UpdateStatus::Available(_) | UpdateStatus::ReadyToRestart => cx.theme().foreground,
        UpdateStatus::Error(_) => gpui::rgb(0x00cc_3333).into(),
        _ => cx.theme().muted_foreground,
    };

    let action_button: Option<Button> = match &board.update_manager.status {
        UpdateStatus::Available(_) => Some(
            Button::new("update-download-button")
                .small()
                .primary()
                .label(I18n::translate(cx, "update_download"))
                .on_click(cx.listener(|board, _, _, cx| board.download_and_install_update(cx))),
        ),
        UpdateStatus::ReadyToRestart => Some(
            Button::new("update-restart-button")
                .small()
                .primary()
                .label(I18n::translate(cx, "update_restart_button"))
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
                .label(I18n::translate(cx, "update_check_now"))
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

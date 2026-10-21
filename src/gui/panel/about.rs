use gpui_kit::{
    Context, ImageSource, Resource, StatefulInteractiveElement,
    component::{
        ActiveTheme, Icon, Sizable,
        button::{Button, ButtonVariants},
        h_flex, v_flex,
    },
    div, img,
    prelude::{FluentBuilder, InteractiveElement, IntoElement, ParentElement, Styled},
    px,
};
use rust_i18n::t;

use crate::{
    gui::{
        board::RopyBoard,
        panel::common::{panel_back_button, panel_header_with_back},
    },
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
            .accessibility_label(tooltip.clone())
            .tooltip(tooltip)
            .on_click(move |_, _, cx| {
                cx.open_url(url);
            }),
    )
}

pub(crate) fn render_about_content(
    board: &RopyBoard,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let version = env!("CARGO_PKG_VERSION");

    let header = panel_header_with_back(
        t!("about_title"),
        panel_back_button("back-button")
            .on_click(cx.listener(|board, _, window, cx| {
                board.active_panel = crate::gui::board::ActivePanel::ClipboardList;
                window.focus(&board.focus_handle, cx);
                cx.notify();
            }))
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            }),
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
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .child(t!("about_version", version = version)),
            )
            // Update section
            .child(render_update_section(board, cx))
            // Description
            .child(
                div()
                    .px_8()
                    .text_center()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(t!("about_description")),
            )
            // License
            .child(
                Button::new("license-button")
                    .small()
                    .ghost()
                    .label(t!("about_license"))
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
                        t!("about_github_tooltip").into_owned(),
                        GITHUB_URL,
                    ))
                    .child(render_social_link_button(
                        "xiaohongshu-button",
                        Icon::empty().path("icons/xiaohongshu.svg").size(px(32.0)),
                        t!("about_xiaohongshu_tooltip").into_owned(),
                        XIAOHONGSHU_URL,
                    )),
            ),
    )
}

fn render_update_section(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let status_text: gpui_kit::SharedString = match &board.update_manager.status {
        UpdateStatus::Idle => t!("update_check_now").into(),
        UpdateStatus::Checking => t!("update_checking").into(),
        UpdateStatus::Available(info) => t!("update_available", version = &info.version).into(),
        UpdateStatus::UpToDate => t!("update_up_to_date").into(),
        UpdateStatus::Downloading(p) => {
            t!("update_downloading", progress = p * 100.0 : {:.0}).into()
        }
        UpdateStatus::ReadyToRestart => t!("update_restart").into(),
        UpdateStatus::Verifying => t!("update_verifying").into(),
        UpdateStatus::Extracting => t!("update_extracting").into(),
        UpdateStatus::Staging => t!("update_staging").into(),
        UpdateStatus::RolledBack => t!("update_rolled_back").into(),
        UpdateStatus::Restarting => t!("update_restarting").into(),
        UpdateStatus::Error(error) => match error {
            crate::updater::errors::UpdateFailure::Network => {
                t!("update_error_network")
            }
            crate::updater::errors::UpdateFailure::RateLimited => {
                t!("update_error_rate_limited")
            }
            crate::updater::errors::UpdateFailure::Verification => {
                t!("update_error_verification")
            }
            crate::updater::errors::UpdateFailure::Installation => {
                t!("update_error_installation")
            }
            crate::updater::errors::UpdateFailure::Restart => {
                t!("update_error_restart")
            }
            crate::updater::errors::UpdateFailure::Managed => t!("update_managed"),
            crate::updater::errors::UpdateFailure::Release => t!("update_error"),
        }
        .into(),
    };
    // Use `foreground` for every non-error status so the text stays legible
    // under dark themes when the window is in transparent mode, where
    // `muted_foreground` blends into the translucent background.
    let status_color = match &board.update_manager.status {
        UpdateStatus::Error(_) => cx.theme().danger,
        _ => cx.theme().foreground,
    };

    let action_button: Option<Button> = match &board.update_manager.status {
        UpdateStatus::Available(_) if board.update_manager.managed => None,
        UpdateStatus::Available(_) => Some(
            Button::new("update-download-button")
                .small()
                .primary()
                .label(t!("update_download"))
                .on_click(cx.listener(|board, _, _, cx| board.download_update(cx))),
        ),
        UpdateStatus::ReadyToRestart
        | UpdateStatus::Error(crate::updater::errors::UpdateFailure::Restart) => Some(
            Button::new("update-restart-button")
                .small()
                .primary()
                .label(t!("update_restart_button"))
                .on_click(cx.listener(|board, _, _, cx| board.restart_after_update(cx))),
        ),
        UpdateStatus::Error(_)
            if board.update_manager.release.is_some() && !board.update_manager.managed =>
        {
            Some(
                Button::new("update-retry-button")
                    .small()
                    .label(t!("update_retry"))
                    .on_click(cx.listener(|board, _, _, cx| board.download_update(cx))),
            )
        }
        UpdateStatus::Idle
        | UpdateStatus::UpToDate
        | UpdateStatus::RolledBack
        | UpdateStatus::Error(_) => Some(
            Button::new("update-check-button")
                .small()
                .ghost()
                .label(t!("update_check_now"))
                .on_click(cx.listener(|board, _, _, cx| board.check_for_update_async(cx))),
        ),
        _ => None,
    };

    v_flex()
        .gap_2()
        .child(
            h_flex()
                .flex_wrap()
                .gap_2()
                .child(div().text_xs().text_color(status_color).child(status_text))
                .children(action_button),
        )
        .when(board.update_manager.managed, |this| {
            this.child(div().text_xs().child(t!("update_managed")))
        })
}

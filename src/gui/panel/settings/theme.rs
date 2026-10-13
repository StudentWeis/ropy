//! Compact appearance controls, composed from the standard Switch and Select.

use gpui_kit::{
    Context, IntoElement, ParentElement, Styled,
    component::{
        ActiveTheme, Icon, IconName, Sizable, h_flex, select::Select, switch::Switch, v_flex,
    },
    div,
};

use crate::{
    gui::{
        board::{RopyBoard, ThemeTarget},
        settings::GlobalSettings,
        theme::ThemeMode,
    },
    i18n::I18n,
};

pub(super) fn render_theme_settings(
    board: &RopyBoard,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let follows_system = GlobalSettings::read(cx, |settings| settings.appearance.follow_system);
    let hint = if follows_system {
        match board.settings_editor.theme.system_mode() {
            ThemeMode::Light => I18n::translate(cx, "settings_system_appearance_light"),
            ThemeMode::Dark => I18n::translate(cx, "settings_system_appearance_dark"),
        }
    } else {
        I18n::translate(cx, "settings_fixed_theme_hint")
    };
    let label = I18n::translate(cx, "settings_follow_system");
    let follow_row = h_flex()
        .w_full()
        .gap_3()
        .py_3()
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_1()
                .child(div().text_sm().child(label.clone()))
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(hint),
                ),
        )
        .child(
            Switch::new("follow-system-toggle")
                .small()
                .checked(follows_system)
                .accessibility_label(label)
                .on_change(cx.listener(|board, enabled, window, cx| {
                    board.save_follow_system(*enabled, window, cx);
                })),
        );

    let choices = if follows_system {
        v_flex()
            .child(theme_row(
                board,
                ThemeTarget::Light,
                I18n::translate(cx, "settings_light_theme"),
                Some(IconName::Sun),
                cx,
            ))
            .child(theme_row(
                board,
                ThemeTarget::Dark,
                I18n::translate(cx, "settings_dark_theme"),
                Some(IconName::Moon),
                cx,
            ))
    } else {
        v_flex().child(theme_row(
            board,
            ThemeTarget::Fixed,
            I18n::translate(cx, "settings_theme"),
            None,
            cx,
        ))
    };
    v_flex().w_full().child(follow_row).child(choices.pb_1())
}

fn theme_row(
    board: &RopyBoard,
    target: ThemeTarget,
    label: String,
    icon: Option<IconName>,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let id = match target {
        ThemeTarget::Fixed => "theme-select",
        ThemeTarget::Light => "light-theme-select",
        ThemeTarget::Dark => "dark-theme-select",
    };
    h_flex()
        .w_full()
        .gap_2()
        .py_2()
        .child(
            h_flex()
                .flex_1()
                .min_w_0()
                .gap_2()
                .text_sm()
                .children(icon.map(|icon| {
                    Icon::new(icon)
                        .small()
                        .text_color(cx.theme().muted_foreground)
                }))
                .child(label.clone()),
        )
        .child(
            div().w_40().flex_shrink_0().child(
                Select::new(board.settings_editor.theme.select(target))
                    .id(id)
                    .small()
                    .w_full()
                    .accessibility_label(label)
                    // Blend the resting trigger into the settings surface while retaining
                    // the Select's native open/focused border and focus ring.
                    .bg(cx.theme().secondary)
                    .border_color(cx.theme().transparent),
            ),
        )
}

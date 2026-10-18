//! Compact appearance controls, composed from the standard Switch and Select.

use gpui_kit::{
    Context, IntoElement, ParentElement, Styled,
    component::{
        ActiveTheme, Icon, IconName, Sizable,
        button::{Button, ButtonVariants},
        h_flex,
        select::Select,
        switch::Switch,
        v_flex,
    },
};
use rust_i18n::t;

use super::{settings_control, settings_row};
use crate::gui::{
    board::{RopyBoard, ThemeTarget},
    settings::GlobalSettings,
};

pub(super) fn render_theme_settings(
    board: &RopyBoard,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let follows_system = GlobalSettings::read(cx, |settings| settings.appearance.follow_system);
    let label = t!("settings_follow_system");
    let follow_row = settings_row(
        label.clone(),
        Switch::new("follow-system-toggle")
            .checked(follows_system)
            .accessibility_label(label)
            .on_change(cx.listener(|board, enabled, window, cx| {
                board.save_follow_system(*enabled, window, cx);
            })),
        cx,
    );

    let choices = if follows_system {
        v_flex()
            .child(theme_row(
                board,
                ThemeTarget::Light,
                t!("settings_light_theme"),
                Some(IconName::Sun),
                cx,
            ))
            .child(theme_row(
                board,
                ThemeTarget::Dark,
                t!("settings_dark_theme"),
                Some(IconName::Moon),
                cx,
            ))
    } else {
        v_flex().child(theme_row(
            board,
            ThemeTarget::Fixed,
            t!("settings_theme"),
            None,
            cx,
        ))
    };
    v_flex()
        .w_full()
        .child(follow_row)
        .child(choices.pb_1())
        .child(render_user_themes(cx))
}

fn render_user_themes(cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    h_flex().w_full().justify_end().pb_2().child(
        Button::new("themes-customize")
            .small()
            .ghost()
            .label(t!("themes_customize"))
            .on_click(cx.listener(|board, _, window, cx| board.open_themes(window, cx))),
    )
}

fn theme_row(
    board: &RopyBoard,
    target: ThemeTarget,
    label: impl Into<gpui_kit::SharedString>,
    icon: Option<IconName>,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let label = label.into();
    let id = match target {
        ThemeTarget::Fixed => "theme-select",
        ThemeTarget::Light => "light-theme-select",
        ThemeTarget::Dark => "dark-theme-select",
    };
    settings_row(
        h_flex()
            .gap_2()
            .children(icon.map(|icon| {
                Icon::new(icon)
                    .small()
                    .text_color(cx.theme().muted_foreground)
            }))
            .child(label.clone()),
        settings_control(
            Select::new(board.settings_editor.theme.select(target))
                .id(id)
                .accessibility_label(label),
        ),
        cx,
    )
}

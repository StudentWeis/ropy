//! Compact appearance controls, composed from the standard Switch and Select.

use gpui_kit::{
    Context, IntoElement, ParentElement, Styled,
    component::{
        ActiveTheme, Disableable, Icon, IconName, Sizable,
        button::{Button, ButtonVariants},
        h_flex,
        menu::{DropdownMenu, PopupMenuItem},
        select::Select,
        switch::Switch,
        v_flex,
    },
    div,
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
        .child(render_user_themes(board, cx))
}

fn render_user_themes(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let catalog = crate::gui::theme::ThemeCatalog::global(cx);
    let busy = board.settings_editor.theme.busy;
    let owner = cx.entity().downgrade();
    let fallback = GlobalSettings::read(cx, |settings| {
        let selected = if settings.appearance.follow_system {
            [
                &settings.appearance.light_theme,
                &settings.appearance.dark_theme,
            ]
            .to_vec()
        } else {
            vec![&settings.theme]
        };
        selected.into_iter().any(|id| catalog.get(id).is_none())
            || (settings.appearance.follow_system
                && (catalog.for_mode(
                    &settings.appearance.light_theme,
                    crate::gui::theme::ThemeMode::Light,
                ) != settings.appearance.light_theme
                    || catalog.for_mode(
                        &settings.appearance.dark_theme,
                        crate::gui::theme::ThemeMode::Dark,
                    ) != settings.appearance.dark_theme))
    });
    v_flex()
        .w_full()
        .child(
            h_flex()
                .flex_wrap()
                .gap_2()
                .child(
                    Button::new("themes-open-folder")
                        .flex_shrink_0()
                        .small()
                        .outline()
                        .label(t!("themes_open_folder"))
                        .disabled(busy)
                        .on_click(cx.listener(|board, _, window, cx| {
                            board.open_themes_folder(window, cx);
                        })),
                )
                .child(
                    Button::new("themes-reload")
                        .flex_shrink_0()
                        .small()
                        .outline()
                        .label(t!("themes_reload"))
                        .disabled(busy)
                        .on_click(cx.listener(|board, _, _, cx| board.reload_themes(cx))),
                )
                .child(
                    Button::new("themes-more")
                        .small()
                        .ghost()
                        .label(t!("themes_more"))
                        .disabled(busy)
                        .dropdown_menu(move |menu, _, _| {
                            let owner = owner.clone();
                            menu.item(PopupMenuItem::new(t!("themes_export")).on_click(
                                move |_, window, cx| {
                                    let _ = owner
                                        .update(cx, |board, cx| board.export_theme(window, cx));
                                },
                            ))
                        }),
                ),
        )
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(t!("themes_help")),
        )
        .children(fallback.then(|| div().text_sm().child(t!("themes_fallback"))))
        .children(
            board
                .settings_editor
                .theme
                .message
                .iter()
                .map(|message| div().text_sm().child(message.clone())),
        )
        .children((!catalog.diagnostics.is_empty()).then(|| {
            v_flex()
                .gap_1()
                .child(div().text_sm().child(t!("themes_errors")))
                .children(
                    catalog
                        .diagnostics
                        .iter()
                        .map(|message| div().text_sm().child(message.clone())),
                )
        }))
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

//! Theme selection and editing share a local preview, with explicit application.
use super::common::{panel_back_button, panel_header_with_back};
use crate::gui::{
    board::{RopyBoard, theme_workspace::ThemeDraft},
    settings::GlobalSettings,
    theme::{ThemeCatalog, document::ColorRole},
};
use gpui_kit::{
    Context, InteractiveElement, IntoElement, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, TestSupportExt,
    component::{
        ActiveTheme, Disableable, Selectable, Sizable,
        button::{Button, ButtonVariants},
        h_flex,
        input::Input,
        menu::{DropdownMenu, PopupMenuItem},
        switch::Switch,
        v_flex,
    },
    div,
    prelude::FluentBuilder,
    rgb,
};
use rust_i18n::t;

pub(crate) fn render_themes(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let Some(workspace) = &board.theme_workspace else {
        return div().into_any_element();
    };
    let header = panel_header_with_back(
        t!("themes_title"),
        panel_back_button("themes-back")
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
            .on_click(cx.listener(|board, _, window, cx| board.leave_themes(window, cx))),
        cx,
    );
    let content = workspace.draft.as_ref().map_or_else(
        || render_catalog(board, cx).into_any_element(),
        |draft| render_editor(draft, cx).into_any_element(),
    );
    v_flex()
        .flex_1()
        .min_h_0()
        .w_full()
        .child(header)
        .child(render_preview(board, cx))
        .child(
            v_flex()
                .id("themes-content")
                .test_support()
                .overflow_y_scroll()
                .flex_1()
                .min_h_0()
                .gap_3()
                .p_2()
                .child(content),
        )
        .child(
            v_flex()
                .flex_shrink_0()
                .gap_2()
                .p_2()
                .border_t_1()
                .border_color(cx.theme().border)
                .when(board.theme_files.busy, |footer| {
                    footer.child(div().text_sm().child(t!("themes_working")))
                })
                .children(workspace.error.iter().map(|error| {
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error.clone())
                }))
                .when(
                    GlobalSettings::read(cx, |settings| settings.appearance.follow_system),
                    |footer| footer.child(div().text_xs().child(t!("themes_follow_hint"))),
                )
                .child(if workspace.draft.is_some() {
                    h_flex()
                        .justify_end()
                        .gap_2()
                        .child(
                            Button::new("theme-cancel")
                                .label(t!("themes_cancel"))
                                .on_click(cx.listener(|board, _, window, cx| {
                                    board.leave_themes(window, cx);
                                })),
                        )
                        .child(
                            Button::new("theme-save")
                                .primary()
                                .label(t!("themes_save"))
                                .disabled(board.theme_files.busy)
                                .on_click(cx.listener(|board, _, window, cx| {
                                    board.save_theme_draft(window, cx);
                                })),
                        )
                } else {
                    h_flex()
                        .justify_between()
                        .gap_2()
                        .child(theme_menu(board.theme_files.busy, cx))
                        .child(
                            Button::new("theme-use")
                                .primary()
                                .label(t!("themes_use"))
                                .disabled(board.theme_files.busy)
                                .on_click(cx.listener(|board, _, window, cx| {
                                    board.use_selected_theme(window, cx);
                                })),
                        )
                }),
        )
        .into_any_element()
}

fn render_catalog(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let selected = board
        .theme_workspace
        .as_ref()
        .map(|workspace| &workspace.selected);
    v_flex()
        .gap_2()
        .child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("theme-new")
                        .small()
                        .label(t!("themes_new"))
                        .disabled(board.theme_files.busy)
                        .on_click(cx.listener(|board, _, window, cx| {
                            board.new_theme_draft(true, window, cx);
                        })),
                )
                .when(
                    selected.is_some_and(|id| id.code().starts_with("user:")),
                    |row| {
                        row.child(
                            Button::new("theme-edit")
                                .small()
                                .label(t!("themes_edit"))
                                .disabled(board.theme_files.busy)
                                .on_click(cx.listener(|board, _, window, cx| {
                                    board.new_theme_draft(false, window, cx);
                                })),
                        )
                    },
                ),
        )
        .children(ThemeCatalog::global(cx).definitions().map(|theme| {
            let id = theme.id().clone();
            let palette = theme.palette();
            Button::new(SharedString::from(format!("theme-choice-{}", id.code())))
                .w_full()
                .outline()
                .selected(selected == Some(&id))
                .label(theme.name().to_owned())
                .child(
                    h_flex().gap_1().children(
                        [palette.background, palette.foreground, palette.primary]
                            .into_iter()
                            .map(|color| div().size_3().rounded_full().bg(rgb(color))),
                    ),
                )
                .on_click(cx.listener(move |board, _, _, cx| {
                    if let Some(workspace) = &mut board.theme_workspace {
                        workspace.selected = id.clone();
                        workspace.error = None;
                    }
                    cx.notify();
                }))
        }))
        .children(
            board
                .theme_files
                .message
                .iter()
                .chain(ThemeCatalog::global(cx).diagnostics.iter())
                .map(|message| div().text_sm().child(message.clone())),
        )
}

fn render_editor(draft: &ThemeDraft, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    v_flex()
        .gap_2()
        .child(
            v_flex()
                .gap_1()
                .child(div().text_sm().child(t!("themes_name")))
                .child(Input::new(&draft.name).aria_label(t!("themes_name"))),
        )
        .child(
            h_flex().justify_between().child(t!("themes_dark")).child(
                Switch::new("theme-mode")
                    .checked(draft.dark)
                    .accessibility_label(t!("themes_dark"))
                    .on_change(cx.listener(|board, enabled, _, cx| {
                        if let Some(draft) = board
                            .theme_workspace
                            .as_mut()
                            .and_then(|workspace| workspace.draft.as_mut())
                        {
                            draft.dark = *enabled;
                        }
                        cx.notify();
                    })),
            ),
        )
        .children(
            draft
                .colors
                .iter()
                .take(if draft.advanced {
                    draft.colors.len()
                } else {
                    6
                })
                .map(|color| {
                    h_flex()
                        .justify_between()
                        .gap_2()
                        .child(div().text_sm().child(t!(color.role.label_key())))
                        .child(div().w(gpui_kit::rems(9.)).child(color.editor.clone()))
                }),
        )
        .child(
            Button::new("theme-advanced")
                .small()
                .ghost()
                .label(if draft.advanced {
                    t!("themes_less")
                } else {
                    t!("themes_advanced")
                })
                .on_click(cx.listener(|board, _, _, cx| {
                    if let Some(draft) = board
                        .theme_workspace
                        .as_mut()
                        .and_then(|workspace| workspace.draft.as_mut())
                    {
                        draft.advanced = !draft.advanced;
                    }
                    cx.notify();
                })),
        )
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(t!("themes_rgb_hint")),
        )
}

fn render_preview(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let Some(workspace) = &board.theme_workspace else {
        return div();
    };
    let Some(theme) = ThemeCatalog::global(cx).get(&workspace.selected) else {
        return div();
    };
    let document = workspace.draft.as_ref().map(|draft| draft.preview(cx));
    let palette = document
        .as_ref()
        .map_or_else(|| theme.palette(), |document| &document.palette);
    let color = |role: ColorRole| rgb(role.get(palette));
    let current = workspace.draft.is_none()
        && GlobalSettings::read(cx, |settings| {
            ThemeCatalog::global(cx).resolve(settings, board.settings_editor.theme.system_mode())
                == workspace.selected
        });
    let status = if current {
        t!("themes_current")
    } else {
        t!("themes_preview")
    };
    let role = workspace
        .draft
        .as_ref()
        .map_or(ColorRole::Background, |draft| draft.preview_role);
    let (surface, foreground) = preview_surface(role);
    let (button, button_text) = preview_button(role);
    v_flex()
        .flex_shrink_0()
        .mx_2()
        .mb_2()
        .p_3()
        .gap_2()
        .rounded_lg()
        .border_1()
        .border_color(color(if role == ColorRole::Ring {
            ColorRole::Ring
        } else {
            ColorRole::Border
        }))
        .bg(color(ColorRole::Background))
        .text_color(color(ColorRole::Foreground))
        .child(
            h_flex()
                .justify_between()
                .text_xs()
                .child(
                    div()
                        .id("themes-preview-status")
                        .test_support()
                        .aria_label(status.clone())
                        .child(status),
                )
                .children(
                    workspace
                        .draft
                        .as_ref()
                        .map(|draft| div().child(t!(draft.preview_role.label_key()))),
                ),
        )
        .child(
            v_flex()
                .gap_1()
                .rounded_md()
                .p_2()
                .bg(color(surface))
                .text_color(color(foreground))
                .child(
                    div()
                        .p_1()
                        .rounded_sm()
                        .bg(color(ColorRole::Input))
                        .child(t!("themes_preview_search")),
                )
                .child(
                    h_flex()
                        .justify_between()
                        .child(t!("themes_preview_item"))
                        .child(
                            div()
                                .w_1()
                                .h_4()
                                .rounded_full()
                                .bg(color(ColorRole::ScrollbarThumb)),
                        ),
                ),
        )
        .child(
            h_flex().justify_end().child(
                div()
                    .rounded_md()
                    .px_3()
                    .py_1()
                    .bg(color(button))
                    .text_color(color(button_text))
                    .child(t!("themes_preview_action")),
            ),
        )
}

const fn preview_surface(role: ColorRole) -> (ColorRole, ColorRole) {
    match role {
        ColorRole::Accent | ColorRole::AccentForeground => {
            (ColorRole::Accent, ColorRole::AccentForeground)
        }
        ColorRole::Muted | ColorRole::MutedForeground => {
            (ColorRole::Muted, ColorRole::MutedForeground)
        }
        ColorRole::Popover | ColorRole::PopoverForeground => {
            (ColorRole::Popover, ColorRole::PopoverForeground)
        }
        ColorRole::Selection | ColorRole::ListHover | ColorRole::ListActive => {
            (role, ColorRole::Foreground)
        }
        _ => (ColorRole::Secondary, ColorRole::SecondaryForeground),
    }
}

fn theme_menu(busy: bool, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let owner = cx.entity().downgrade();
    Button::new("themes-more")
        .ghost()
        .label(t!("themes_more"))
        .disabled(busy)
        .dropdown_menu(move |menu, _, _| {
            let export = owner.clone();
            let folder = owner.clone();
            let reload = owner.clone();
            menu.item(
                PopupMenuItem::new(t!("themes_export")).on_click(move |_, window, cx| {
                    let _ = export.update(cx, |board, cx| board.export_theme(window, cx));
                }),
            )
            .item(
                PopupMenuItem::new(t!("themes_open_folder")).on_click(move |_, window, cx| {
                    let _ = folder.update(cx, |board, cx| board.open_themes_folder(window, cx));
                }),
            )
            .item(
                PopupMenuItem::new(t!("themes_reload")).on_click(move |_, _, cx| {
                    let _ = reload.update(cx, RopyBoard::reload_themes);
                }),
            )
        })
}

const fn preview_button(role: ColorRole) -> (ColorRole, ColorRole) {
    match role {
        ColorRole::PrimaryHover => (ColorRole::PrimaryHover, ColorRole::PrimaryForeground),
        ColorRole::PrimaryActive => (ColorRole::PrimaryActive, ColorRole::PrimaryForeground),
        ColorRole::Danger | ColorRole::DangerForeground => {
            (ColorRole::Danger, ColorRole::DangerForeground)
        }
        _ => (ColorRole::Primary, ColorRole::PrimaryForeground),
    }
}

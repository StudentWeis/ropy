use gpui_kit::{
    Context, SharedString, StatefulInteractiveElement,
    base::TestSupportExt,
    component::{
        ActiveTheme, Disableable, Sizable, StyledExt,
        button::{Button, ButtonVariants},
        h_flex,
        kbd::Kbd,
        menu::{DropdownMenu, PopupMenuItem},
        v_flex,
    },
    div,
    prelude::{FluentBuilder, InteractiveElement, IntoElement, ParentElement, Styled},
};
use rust_i18n::t;

use super::common::{panel_back_button, panel_header_with_back};
use crate::{
    config::Settings,
    gui::{
        board::{
            RopyBoard,
            actions::shortcuts::{HistoryShortcut, ShortcutGroup, history_shortcuts},
            shortcut_editor::ShortcutEditor,
        },
        settings::GlobalSettings,
    },
};

pub(crate) fn render_shortcuts(board: &RopyBoard, cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    let Some(editor) = &board.shortcut_editor else {
        return v_flex();
    };
    let header = panel_header_with_back(
        t!("shortcuts_title"),
        panel_back_button("shortcuts-back")
            .on_click(cx.listener(|board, _, window, cx| board.leave_shortcuts(window, cx)))
            .on_mouse_down(gpui_kit::MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            }),
        cx,
    );
    let disabled = GlobalSettings::read(cx, Settings::is_recovery_required);
    let shortcuts = history_shortcuts();
    let mut content = v_flex()
        .id("shortcuts-content")
        .test_support()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .gap_2()
        .pb_3()
        .child(
            div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(t!("shortcuts_description")),
        );
    for (group, title) in [
        (ShortcutGroup::Common, t!("shortcuts_common")),
        (ShortcutGroup::Navigation, t!("shortcuts_navigation")),
        (
            ShortcutGroup::QuickSelection,
            t!("shortcuts_quick_selection"),
        ),
        (ShortcutGroup::Window, t!("shortcuts_window")),
    ] {
        content = content.child(
            div()
                .pt_3()
                .text_xs()
                .font_medium()
                .text_color(cx.theme().muted_foreground)
                .child(title),
        );
        for shortcut in shortcuts.iter().filter(|s| s.group == group) {
            content = content.child(shortcut_row(shortcut, board, disabled, cx));
        }
    }
    // Preserve unknown entries from hand-edited files until explicitly removed.
    for id in editor
        .draft
        .history
        .keys()
        .filter(|id| !shortcuts.iter().any(|s| &s.id == *id))
    {
        let action = id.clone();
        content = content.child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(t!("keybindings_unknown_action", action = id)),
                )
                .child(
                    Button::new(SharedString::from(format!("shortcuts-remove-unknown-{id}")))
                        .small()
                        .label(t!("shortcuts_remove_unknown"))
                        .disabled(disabled)
                        .on_click(cx.listener(move |board, _, _, cx| {
                            board.edit_shortcuts(
                                |editor| {
                                    editor.edit(|draft| {
                                        draft.history.remove(&action);
                                    });
                                },
                                cx,
                            );
                        })),
                ),
        );
    }
    let footer = shortcut_footer(editor, disabled, cx);
    v_flex()
        .flex_1()
        .min_h_0()
        .w_full()
        .child(header)
        .child(content)
        .child(footer)
}

fn shortcut_row(
    shortcut: &HistoryShortcut,
    board: &RopyBoard,
    disabled: bool,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let Some(editor) = &board.shortcut_editor else {
        return v_flex();
    };
    let id = &shortcut.id;
    let recording = editor.recording.as_ref().is_some_and(|r| r.action == *id);
    let menu = shortcut_menu(shortcut, disabled || editor.recording.is_some(), cx);
    let keys = editor.keys(shortcut);
    let controls = shortcut_controls(shortcut, &keys, disabled || editor.recording.is_some(), cx);
    v_flex()
        .w_full()
        .gap_1()
        .p_2()
        .rounded_md()
        .bg(cx.theme().secondary)
        .child(
            h_flex()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .child(shortcut.label.clone()),
                )
                .child(menu),
        )
        .when(keys.is_empty(), |row| {
            row.child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(t!("shortcuts_unassigned")),
            )
        })
        .child(controls)
        .when(recording, |row| {
            row.child(
                v_flex()
                    .gap_1()
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().primary)
                            .child(t!("shortcuts_record_prompt")),
                    )
                    .child(
                        Button::new(SharedString::from(format!("shortcuts-cancel-{id}")))
                            .small()
                            .label(t!("shortcuts_cancel_recording"))
                            .on_click(
                                cx.listener(|board, _, _, cx| board.cancel_shortcut_recording(cx)),
                            ),
                    ),
            )
        })
        .when_some(
            editor.error.as_ref().filter(|error| error.affects(id)),
            |row, error| {
                let message = error.message();
                row.child(
                    div()
                        .id(SharedString::from(format!("shortcuts-error-{id}")))
                        .test_support()
                        .aria_label(message.clone())
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(message),
                )
            },
        )
}

fn shortcut_footer(
    editor: &ShortcutEditor,
    disabled: bool,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let dirty = editor.is_dirty();
    let recording = editor.recording.is_some();
    v_flex()
        .id("shortcuts-footer")
        .flex_shrink_0()
        .pt_2()
        .gap_2()
        .border_t_1()
        .border_color(cx.theme().border)
        .when_some(editor.save_error.clone(), |footer, message| {
            footer.child(
                div()
                    .id("shortcuts-save-error")
                    .test_support()
                    .aria_label(message.clone())
                    .text_sm()
                    .text_color(cx.theme().danger)
                    .child(message),
            )
        })
        .child(
            div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(if editor.error.is_some() {
                    t!("shortcuts_fix_errors")
                } else if dirty {
                    t!("shortcuts_unsaved")
                } else {
                    t!("shortcuts_saved")
                }),
        )
        .child(
            h_flex()
                .gap_2()
                .justify_between()
                .child(
                    Button::new("shortcuts-defaults")
                        .small()
                        .ghost()
                        .label(t!("shortcuts_restore_all"))
                        .disabled(disabled || recording)
                        .on_click(cx.listener(|board, _, _, cx| {
                            board.edit_shortcuts(
                                |editor| editor.edit(|draft| draft.history.clear()),
                                cx,
                            );
                        })),
                )
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("shortcuts-revert")
                                .small()
                                .label(t!("shortcuts_revert"))
                                .disabled(!dirty || recording)
                                .on_click(cx.listener(|board, _, _, cx| {
                                    board.edit_shortcuts(ShortcutEditor::revert, cx);
                                })),
                        )
                        .child(
                            Button::new("shortcuts-apply")
                                .small()
                                .primary()
                                .label(t!("shortcuts_apply"))
                                .disabled(disabled || !dirty || recording || editor.error.is_some())
                                .on_click(cx.listener(|board, _, _, cx| board.apply_shortcuts(cx))),
                        ),
                ),
        )
}

fn shortcut_menu(
    shortcut: &HistoryShortcut,
    disabled: bool,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let id = &shortcut.id;
    let owner = cx.entity().downgrade();
    let restore_id = id.clone();
    let clear_id = id.clone();
    Button::new(SharedString::from(format!("shortcuts-menu-{id}")))
        .small()
        .ghost()
        .label(t!("shortcuts_more"))
        .accessibility_label(t!("shortcuts_action_options", action = shortcut.label))
        .disabled(disabled)
        .dropdown_menu(move |menu, _, _| {
            let restore_owner = owner.clone();
            let clear_owner = owner.clone();
            let restore_id = restore_id.clone();
            let clear_id = clear_id.clone();
            menu.item(
                PopupMenuItem::new(t!("shortcuts_restore")).on_click(move |_, _, cx| {
                    let _ = restore_owner.update(cx, |board, cx| {
                        board.edit_shortcuts(
                            |editor| {
                                editor.edit(|draft| {
                                    draft.history.remove(&restore_id);
                                });
                            },
                            cx,
                        );
                    });
                }),
            )
            .item(
                PopupMenuItem::new(t!("shortcuts_clear")).on_click(move |_, _, cx| {
                    let _ = clear_owner.update(cx, |board, cx| {
                        board.edit_shortcuts(
                            |editor| {
                                editor.edit(|draft| {
                                    draft.history.insert(clear_id.clone(), Vec::new());
                                });
                            },
                            cx,
                        );
                    });
                }),
            )
        })
}

fn shortcut_controls(
    shortcut: &HistoryShortcut,
    keys: &[String],
    disabled: bool,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let id = &shortcut.id;
    let mut controls = h_flex().flex_wrap().gap_2();
    for key in keys {
        let replace_action = id.clone();
        let previous = key.clone();
        let remove_action = id.clone();
        let mut remaining = keys.to_vec();
        remaining.retain(|other| other != key);
        let label =
            gpui_kit::Keystroke::parse(key).map_or_else(|_| key.clone(), |key| Kbd::format(&key));
        let remove_label = t!("shortcuts_remove_key", key = label, action = shortcut.label);
        controls = controls.child(
            h_flex()
                .flex_shrink_0()
                .border_1()
                .border_color(cx.theme().border)
                .rounded(cx.theme().radius)
                .bg(cx.theme().background)
                .child(
                    Button::new(SharedString::from(format!("shortcuts-key-{id}-{key}")))
                        .small()
                        .ghost()
                        .label(label.clone())
                        .disabled(disabled)
                        .accessibility_label(t!(
                            "shortcuts_change_key",
                            key = label,
                            action = shortcut.label
                        ))
                        .on_click(cx.listener(move |board, _, window, cx| {
                            board.record_shortcut(
                                replace_action.clone(),
                                Some(previous.clone()),
                                window,
                                cx,
                            );
                        })),
                )
                .child(div().w_px().h_4().bg(cx.theme().border))
                .child(
                    Button::new(SharedString::from(format!("shortcuts-remove-{id}-{key}")))
                        .small()
                        .ghost()
                        .label(t!("shortcuts_remove_symbol"))
                        .accessibility_label(remove_label.clone())
                        .tooltip(remove_label)
                        .disabled(disabled)
                        .on_click(cx.listener(move |board, _, _, cx| {
                            board.edit_shortcuts(
                                |editor| {
                                    editor.edit(|draft| {
                                        draft
                                            .history
                                            .insert(remove_action.clone(), remaining.clone());
                                    });
                                },
                                cx,
                            );
                        })),
                ),
        );
    }
    let add_action = id.clone();
    controls = controls.child(
        Button::new(SharedString::from(format!("shortcuts-add-{id}")))
            .small()
            .outline()
            .label(t!("shortcuts_add"))
            .accessibility_label(t!("shortcuts_add_key", action = shortcut.label))
            .disabled(disabled)
            .on_click(cx.listener(move |board, _, window, cx| {
                board.record_shortcut(add_action.clone(), None, window, cx);
            })),
    );
    controls
}

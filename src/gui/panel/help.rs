use gpui_kit::{
    Action, AnyElement, App, AsKeystroke, Context, Keystroke, SharedString,
    StatefulInteractiveElement,
    base::TestSupportExt,
    component::{
        ActiveTheme, Sizable,
        button::{Button, ButtonVariants},
        h_flex,
        kbd::Kbd,
        tag::Tag,
        v_flex,
    },
    div,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
};
use rust_i18n::t;

use crate::{
    config::LayoutMode,
    gui::{
        board::{
            RopyBoard,
            actions::{
                ConfirmRecord, ConfirmSelection, ConfirmSelectionPlainText, CycleFilterNext,
                CycleFilterPrev, DeleteRecord, FocusSearch, Hide, PreviewSelection,
                QUICK_SELECT_COUNT, SelectLeft, SelectNext, SelectPrev, SelectRight,
                ToggleFavorite, ToggleFavoritesFilter, ToggleWindowPin, binding_applies_to_history,
            },
        },
        panel::common::{panel_back_button, panel_header_with_back},
    },
};

/// Width of the key column, shared by the column header and every row.
const KEY_COLUMN_WIDTH: gpui_kit::Rems = gpui_kit::rems(12.0);

/// One registered binding, kept as keystrokes so rows can render key caps.
struct ShortcutBinding {
    keystrokes: Vec<Keystroke>,
    display: String,
    search_scope: bool,
}

struct ShortcutRow {
    bindings: Vec<ShortcutBinding>,
    id: &'static str,
    label: SharedString,
}

impl ShortcutRow {
    fn new(label: impl Into<SharedString>, action: &dyn Action, cx: &App) -> Self {
        Self {
            bindings: Vec::new(),
            id: action.name(),
            label: label.into(),
        }
        .action(action, cx)
    }

    fn action(mut self, action: &dyn Action, cx: &App) -> Self {
        let keymap = cx.key_bindings();
        let keymap = keymap.borrow();
        let mut bindings = keymap.bindings_for_action(action).collect::<Vec<_>>();
        bindings.sort_by_key(|binding| !binding_applies_to_history(binding));
        for binding in bindings {
            let keystrokes = binding
                .keystrokes()
                .iter()
                .map(|key| key.as_keystroke().clone())
                .collect::<Vec<_>>();
            let keys = keystrokes
                .iter()
                .map(Kbd::format)
                .collect::<Vec<_>>()
                .join(" ");
            let search_scope = !binding_applies_to_history(binding);
            let display = if search_scope {
                t!("help_search_binding", keys = keys).to_string()
            } else {
                keys
            };
            if self.bindings.iter().all(|known| known.display != display) {
                self.bindings.push(ShortcutBinding {
                    keystrokes,
                    display,
                    search_scope,
                });
            }
        }
        self
    }
}

/// Key caps for one row, separated the same way as the accessible label.
fn render_row_keys(bindings: Vec<ShortcutBinding>, cx: &App) -> AnyElement {
    h_flex()
        .w_full()
        .flex_wrap()
        .items_center()
        .gap_1()
        .children(
            bindings
                .into_iter()
                .enumerate()
                .flat_map(|(index, binding)| {
                    let mut chips = Vec::with_capacity(2);
                    if index > 0 {
                        chips.push(
                            div()
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .child("/")
                                .into_any_element(),
                        );
                    }
                    let keystrokes = binding.keystrokes;
                    let mut keys = h_flex()
                        .items_center()
                        .gap_0p5()
                        .children(keystrokes.into_iter().map(Kbd::new));
                    if binding.search_scope {
                        keys = keys.child(
                            Tag::secondary()
                                .outline()
                                .small()
                                .child(t!("help_search_scope")),
                        );
                    }
                    chips.push(keys.into_any_element());
                    chips
                }),
        )
        .into_any_element()
}

// Presentation groups refer to actions; key names come exclusively from GPUI's
// registered bindings, including aliases and platform-specific modifiers.
fn shortcut_rows(board: &RopyBoard, cx: &App) -> Vec<ShortcutRow> {
    let mut rows = vec![
        ShortcutRow::new(t!("help_search"), &FocusSearch, cx),
        ShortcutRow::new(t!("help_nav_vertical"), &SelectPrev, cx).action(&SelectNext, cx),
    ];
    if board.layout_mode == LayoutMode::Grid {
        rows.push(
            ShortcutRow::new(t!("help_nav_left_right"), &SelectLeft, cx).action(&SelectRight, cx),
        );
    }
    let quick_select = (1..QUICK_SELECT_COUNT).fold(
        ShortcutRow::new(t!("help_quick_select"), &ConfirmRecord { index: 0 }, cx),
        |row, index| row.action(&ConfirmRecord { index }, cx),
    );
    rows.extend([
        quick_select,
        ShortcutRow::new(t!("help_toggle_preview"), &PreviewSelection, cx),
        ShortcutRow::new(t!("help_confirm"), &ConfirmSelection, cx),
        ShortcutRow::new(
            t!("help_confirm_plain_text"),
            &ConfirmSelectionPlainText,
            cx,
        ),
        ShortcutRow::new(t!("help_delete"), &DeleteRecord, cx),
        ShortcutRow::new(t!("help_favorite"), &ToggleFavorite, cx),
        ShortcutRow::new(t!("help_cycle_filter"), &CycleFilterPrev, cx)
            .action(&CycleFilterNext, cx),
        ShortcutRow::new(
            t!("help_toggle_favorites_filter"),
            &ToggleFavoritesFilter,
            cx,
        ),
    ]);
    if board.can_toggle_window_pin() {
        rows.push(ShortcutRow::new(t!("help_pin"), &ToggleWindowPin, cx));
    }
    rows.push(ShortcutRow::new(t!("help_hide"), &Hide, cx));
    rows.retain(|row| !row.bindings.is_empty());
    rows
}

/// Column header row, using the same column widths as the shortcut rows.
fn render_column_header(cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    h_flex()
        .w_full()
        .px_2()
        .py_1()
        .mb_1()
        .gap_4()
        .rounded_md()
        .bg(cx.theme().secondary)
        .child(
            div()
                .id("help-keys-header")
                .test_support()
                .w(KEY_COLUMN_WIDTH)
                .flex_shrink_0()
                .text_sm()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(cx.theme().secondary_foreground)
                .child(t!("help_key")),
        )
        .child(
            div()
                .id("help-action-header")
                .test_support()
                .flex_1()
                .text_sm()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(cx.theme().secondary_foreground)
                .child(t!("help_action")),
        )
}

pub(crate) fn render_help_content(
    board: &RopyBoard,
    cx: &Context<'_, RopyBoard>,
) -> impl IntoElement {
    let header = panel_header_with_back(
        t!("help_title"),
        panel_back_button("help-back-button")
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

    let col_header = render_column_header(cx);

    // Build shortcut rows
    let rows = shortcut_rows(board, cx)
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            let ShortcutRow {
                bindings,
                id,
                label,
            } = row;
            let keys = bindings
                .iter()
                .map(|binding| binding.display.as_str())
                .collect::<Vec<_>>()
                .join(" / ");

            let row_bg = if i % 2 == 0 {
                cx.theme().background
            } else {
                cx.theme().secondary
            };

            h_flex()
                .id(id)
                .test_support()
                .aria_label(format!("{label}: {keys}"))
                .w_full()
                .px_2()
                .py_1()
                .gap_4()
                .rounded_md()
                .bg(row_bg)
                .child(
                    // Key badges
                    div()
                        .id(SharedString::from(format!("help-keys-{id}")))
                        .test_support()
                        .w(KEY_COLUMN_WIDTH)
                        .flex_shrink_0()
                        .child(render_row_keys(bindings, cx)),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("help-action-{id}")))
                        .test_support()
                        .flex_1()
                        .text_sm()
                        .text_color(cx.theme().foreground)
                        .child(label),
                )
        });

    v_flex()
        .size_full()
        .child(header)
        .child(customize_button(cx))
        .child(
            v_flex()
                .id("help-content")
                .overflow_y_scroll()
                .size_full()
                .flex_1()
                .gap_0p5()
                .pb_4()
                .child(col_header)
                .children(rows),
        )
}

fn customize_button(cx: &Context<'_, RopyBoard>) -> impl IntoElement {
    h_flex().justify_end().pb_2().child(
        Button::new("customize-shortcuts")
            .small()
            .ghost()
            .label(t!("shortcuts_customize"))
            .on_click(cx.listener(|board, _, window, cx| board.open_shortcuts(window, cx))),
    )
}

use gpui_kit::{
    Action, App, AsKeystroke, Context, SharedString, StatefulInteractiveElement,
    base::TestSupportExt,
    component::{
        ActiveTheme, Sizable,
        button::{Button, ButtonVariants},
        h_flex,
        kbd::Kbd,
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

struct ShortcutRow {
    keys: Vec<String>,
    id: &'static str,
    label: SharedString,
}

impl ShortcutRow {
    fn new(label: impl Into<SharedString>, action: &dyn Action, cx: &App) -> Self {
        Self {
            keys: Vec::new(),
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
            let keys = binding
                .keystrokes()
                .iter()
                .map(|key| Kbd::format(key.as_keystroke()))
                .collect::<Vec<_>>()
                .join(" ");
            if !self.keys.contains(&keys) {
                let display = if binding_applies_to_history(binding) {
                    keys
                } else {
                    t!("help_search_binding", keys = keys).to_string()
                };
                if !self.keys.contains(&display) {
                    self.keys.push(display);
                }
            }
        }
        self
    }
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
    rows.retain(|row| !row.keys.is_empty());
    rows
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

    // Column header row
    let col_header = h_flex()
        .w_full()
        .px_2()
        .py_1()
        .mb_1()
        .gap_4()
        .rounded_md()
        .bg(cx.theme().secondary)
        .child(
            div()
                .w_32()
                .flex_shrink_0()
                .text_sm()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(cx.theme().secondary_foreground)
                .child(t!("help_key")),
        )
        .child(
            div()
                .flex_1()
                .text_sm()
                .font_weight(gpui_kit::FontWeight::SEMIBOLD)
                .text_color(cx.theme().secondary_foreground)
                .child(t!("help_action")),
        );

    // Build shortcut rows
    let rows = shortcut_rows(board, cx)
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            let label = row.label;
            let keys = row.keys.join(" / ");

            let row_bg = if i % 2 == 0 {
                cx.theme().background
            } else {
                cx.theme().secondary
            };

            h_flex()
                .id(row.id)
                .test_support()
                .aria_label(format!("{label}: {keys}"))
                .w_full()
                .px_2()
                .py_1()
                .gap_4()
                .rounded_md()
                .bg(row_bg)
                .child(
                    // Key badge
                    div().w_32().flex_shrink_0().child(
                        div()
                            .flex()
                            .items_center()
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(cx.theme().background)
                            .text_xs()
                            .font_family("monospace")
                            .text_color(cx.theme().foreground)
                            .child(keys),
                    ),
                )
                .child(
                    div()
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

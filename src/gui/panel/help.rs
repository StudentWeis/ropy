use gpui::{
    Context, div,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
    px,
};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex, v_flex,
};

use crate::{constants::BACK_ARROW, gui::board::RopyBoard, i18n::I18n};

/// A single row in the shortcuts table
struct ShortcutRow {
    key: &'static str,
    label_key: &'static str,
}

const SHORTCUTS: &[ShortcutRow] = &[
    ShortcutRow {
        key: "/",
        label_key: "help_search",
    },
    ShortcutRow {
        key: "↑ / ↓",
        label_key: "help_nav_up_down",
    },
    ShortcutRow {
        key: "K / J",
        label_key: "help_nav_up_down",
    },
    ShortcutRow {
        key: "1 – 5",
        label_key: "help_quick_select",
    },
    ShortcutRow {
        key: "Space",
        label_key: "help_toggle_preview",
    },
    ShortcutRow {
        key: "Enter",
        label_key: "help_confirm",
    },
    ShortcutRow {
        key: "Delete / D",
        label_key: "help_delete",
    },
    ShortcutRow {
        key: "F",
        label_key: "help_favorite",
    },
    ShortcutRow {
        key: "P",
        label_key: "help_pin",
    },
    ShortcutRow {
        key: "Esc / Q",
        label_key: "help_hide",
    },
];

/// Render the help panel (keyboard shortcuts overview)
#[allow(clippy::too_many_lines)]
pub fn render_help_content(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let header = h_flex()
        .justify_between()
        .items_center()
        .mb_4()
        .pt_4()
        .child(
            Button::new("help-back-button")
                .small()
                .ghost()
                .label(BACK_ARROW)
                .on_click(cx.listener(|board, _, window, cx| {
                    board.active_panel = crate::gui::board::ActivePanel::ClipboardList;
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
                .child(I18n::translate(cx, "help_title")),
        )
        // Spacer to keep the title centered
        .child(div().w(px(55.)));

    #[cfg(target_os = "windows")]
    let header = header.on_mouse_down(gpui::MouseButton::Left, |_, window, _cx| {
        crate::gui::utils::start_window_drag(window);
    });

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
                .w(px(100.))
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(cx.theme().secondary_foreground)
                .child(I18n::translate(cx, "help_key")),
        )
        .child(
            div()
                .flex_1()
                .text_sm()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(cx.theme().secondary_foreground)
                .child(I18n::translate(cx, "help_action")),
        );

    // Build shortcut rows
    let rows = SHORTCUTS
        .iter()
        .filter(|row| board.can_toggle_window_pin() || row.label_key != "help_pin")
        .enumerate()
        .map(|(i, row)| {
            // Resolve label: "help_nav_up_down" needs to combine two keys
            let label = if row.label_key == "help_nav_up_down" {
                // Concat the two individual translated strings
                format!(
                    "{} / {}",
                    I18n::translate(cx, "help_nav_up"),
                    I18n::translate(cx, "help_nav_down")
                )
            } else {
                I18n::translate(cx, row.label_key)
            };

            let row_bg = if i % 2 == 0 {
                cx.theme().background
            } else {
                cx.theme().secondary
            };

            h_flex()
                .w_full()
                .px_2()
                .py_1()
                .gap_4()
                .rounded_md()
                .bg(row_bg)
                .child(
                    // Key badge
                    div().w(px(100.)).child(
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
                            .child(row.key),
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

    v_flex().size_full().child(header).child(
        v_flex()
            .flex_1()
            .w_full()
            .gap_0p5()
            .child(col_header)
            .children(rows),
    )
}

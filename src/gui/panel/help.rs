use gpui::{
    Context, StatefulInteractiveElement, div,
    prelude::{InteractiveElement, IntoElement, ParentElement, Styled},
    px,
};
use gpui_component::{ActiveTheme, h_flex, v_flex};

use crate::{
    config::LayoutMode,
    gui::{
        board::RopyBoard,
        panel::common::{panel_back_button, panel_header_with_back},
    },
    i18n::I18n,
};

/// A single row in the shortcuts table
struct ShortcutRow {
    key: &'static str,
    label_key: &'static str,
    grid_only: bool,
}

const SHORTCUTS: &[ShortcutRow] = &[
    ShortcutRow {
        key: "/",
        label_key: "help_search",
        grid_only: false,
    },
    ShortcutRow {
        key: "↑ / ↓",
        label_key: "help_nav_up_down",
        grid_only: false,
    },
    ShortcutRow {
        key: "K / J",
        label_key: "help_nav_up_down",
        grid_only: false,
    },
    ShortcutRow {
        key: "← / →",
        label_key: "help_nav_left_right",
        grid_only: true,
    },
    ShortcutRow {
        key: "H / L",
        label_key: "help_nav_left_right",
        grid_only: true,
    },
    ShortcutRow {
        key: "1 – 5",
        label_key: "help_quick_select",
        grid_only: false,
    },
    ShortcutRow {
        key: "Space",
        label_key: "help_toggle_preview",
        grid_only: false,
    },
    ShortcutRow {
        key: "Enter",
        label_key: "help_confirm",
        grid_only: false,
    },
    ShortcutRow {
        key: "Shift+Enter",
        label_key: "help_confirm_plain_text",
        grid_only: false,
    },
    ShortcutRow {
        key: "Delete / D",
        label_key: "help_delete",
        grid_only: false,
    },
    ShortcutRow {
        key: "F",
        label_key: "help_favorite",
        grid_only: false,
    },
    ShortcutRow {
        key: "P",
        label_key: "help_pin",
        grid_only: false,
    },
    ShortcutRow {
        key: "Esc / Q",
        label_key: "help_hide",
        grid_only: false,
    },
];

/// Render the help panel (keyboard shortcuts overview)
#[allow(clippy::too_many_lines)]
pub fn render_help_content(board: &RopyBoard, cx: &Context<RopyBoard>) -> impl IntoElement {
    let header = panel_header_with_back(
        I18n::translate(cx, "help_title"),
        panel_back_button("help-back-button")
            .on_click(cx.listener(|board, _, window, cx| {
                board.active_panel = crate::gui::board::ActivePanel::ClipboardList;
                window.focus(&board.focus_handle);
                cx.notify();
            }))
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
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
        .filter(|row| board.layout_mode == LayoutMode::Grid || !row.grid_only)
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

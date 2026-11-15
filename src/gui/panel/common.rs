#[cfg(any(target_os = "windows", target_os = "linux"))]
use gpui_kit::InteractiveElement;
use gpui_kit::{
    Context, Keystroke, SharedString,
    component::{
        ActiveTheme, Sizable,
        button::{Button, ButtonVariants},
        h_flex,
        kbd::Kbd,
    },
    div,
    prelude::{IntoElement, ParentElement, Styled},
};

/// Direction keys read as arrows on every platform, not only macOS.
fn arrow_glyph(key: &str) -> Option<&'static str> {
    match key {
        "up" => Some("\u{2191}"),
        "down" => Some("\u{2193}"),
        "left" => Some("\u{2190}"),
        "right" => Some("\u{2192}"),
        _ => None,
    }
}

/// Copy of a keystroke whose arrow keys carry the arrow glyph.
pub(super) fn display_keystroke(key: &Keystroke) -> Keystroke {
    let Some(glyph) = arrow_glyph(&key.key) else {
        return key.clone();
    };
    let mut display = key.clone();
    glyph.clone_into(&mut display.key);
    display
}

/// Key label built from the display keystroke.
pub(super) fn keystroke_label(key: &Keystroke) -> String {
    Kbd::format(&display_keystroke(key))
}

/// Balanced side slots keep the title centered whatever the leading and
/// trailing controls measure.
fn panel_header<T, L, R>(
    title: impl Into<SharedString>,
    leading: L,
    trailing: R,
    cx: &Context<'_, T>,
) -> impl IntoElement
where
    L: IntoElement,
    R: IntoElement,
{
    let header = h_flex()
        .items_center()
        .mb_4()
        .pt_4()
        .child(div().flex_1().flex().items_center().child(leading))
        .child(
            div()
                .flex_shrink_0()
                .text_center()
                .text_lg()
                .text_color(cx.theme().foreground)
                .font_weight(gpui_kit::FontWeight::BOLD)
                .child(title.into()),
        )
        .child(div().flex_1().flex().justify_end().child(trailing));

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    let header = header.on_mouse_down(gpui_kit::MouseButton::Left, |_, window, _cx| {
        crate::gui::utils::start_window_drag(window);
    });

    header
}

pub(super) fn panel_back_button(id: &'static str) -> Button {
    Button::new(id)
        .small()
        .ghost()
        .label(crate::constants::BACK_ARROW)
        .accessibility_label(rust_i18n::t!("back"))
}

pub(super) fn panel_header_with_back<T, B>(
    title: impl Into<SharedString>,
    back_button: B,
    cx: &Context<'_, T>,
) -> impl IntoElement
where
    B: IntoElement,
{
    panel_header(title, back_button, div(), cx)
}

/// Panel header with a back control on the left and an action on the right.
pub(super) fn panel_header_with_back_and_action<T, B, A>(
    title: impl Into<SharedString>,
    back_button: B,
    action: A,
    cx: &Context<'_, T>,
) -> impl IntoElement
where
    B: IntoElement,
    A: IntoElement,
{
    panel_header(title, back_button, action, cx)
}

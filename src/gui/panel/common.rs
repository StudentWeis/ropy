use gpui::{
    Context, SharedString, div,
    prelude::{IntoElement, ParentElement, Styled},
    px,
};
use gpui_component::{
    ActiveTheme, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
};

const PANEL_SIDE_WIDTH_PX: f32 = 72.0;

fn panel_side_slot<C: IntoElement>(child: C) -> impl IntoElement {
    div()
        .w(px(PANEL_SIDE_WIDTH_PX))
        .flex()
        .items_start()
        .child(child)
}

fn panel_side_spacer() -> impl IntoElement {
    div().w(px(PANEL_SIDE_WIDTH_PX))
}

fn panel_header<T, L, R>(
    title: impl Into<SharedString>,
    leading: L,
    trailing: R,
    cx: &Context<T>,
) -> impl IntoElement
where
    L: IntoElement,
    R: IntoElement,
{
    let header = h_flex()
        .justify_between()
        .items_center()
        .mb_4()
        .pt_4()
        .child(leading)
        .child(
            div()
                .flex_1()
                .text_center()
                .text_lg()
                .text_color(cx.theme().foreground)
                .font_weight(gpui::FontWeight::BOLD)
                .child(title.into()),
        )
        .child(trailing);

    #[cfg(target_os = "windows")]
    let header = header.on_mouse_down(gpui::MouseButton::Left, |_, window, _cx| {
        crate::gui::utils::start_window_drag(window);
    });

    header
}

pub fn panel_back_button(id: &'static str) -> Button {
    Button::new(id)
        .small()
        .ghost()
        .label(crate::constants::BACK_ARROW)
}

pub fn panel_header_with_back<T, B>(
    title: impl Into<SharedString>,
    back_button: B,
    cx: &Context<T>,
) -> impl IntoElement
where
    B: IntoElement,
{
    panel_header(title, panel_side_slot(back_button), panel_side_spacer(), cx)
}

/// Custom tooltip preview implementation that supports automatic line wrapping
use gpui::{AnyView, App, AppContext, IntoElement, ParentElement, Render, Styled, StyledImage, Window, div, img, px};
use gpui_component::ActiveTheme;
use std::path::PathBuf;

/// Create a tooltip preview that supports automatic line wrapping
///
/// This implementation returns a View that will be correctly rendered by GPUI's tooltip system
///
/// # Usage Example
/// ```rust
/// div()
///     .tooltip(|window, cx| {
///         simple_tooltip("This is tooltip content", window, cx)
///     })
/// ```
pub fn simple_tooltip(content: impl Into<String>, window: &mut Window, cx: &mut App) -> AnyView {
    let content = content.into();
    let window_width = window.bounds().size.width;
    let max_width = (window_width - px(40.0)).into();

    cx.new(move |_cx| TooltipView { content, max_width }).into()
}

struct TooltipView {
    content: String,
    max_width: f32,
}

impl Render for TooltipView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .bg(cx.theme().popover)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .shadow_lg()
            .px_3()
            .py_2()
            .max_w(px(self.max_width))
            .min_w_0()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .text_color(cx.theme().popover_foreground)
                    .line_height(gpui::relative(1.5))
                    .overflow_hidden()
                    .child(self.content.clone()),
            )
    }
}

/// Create an image tooltip preview
///
/// # Usage Example
/// ```rust
/// div()
///     .tooltip(|window, cx| {
///         image_tooltip("/path/to/image.png", window, cx)
///     })
/// ```
pub fn image_tooltip(image_path: impl Into<String>, window: &mut Window, cx: &mut App) -> AnyView {
    let image_path = image_path.into();
    let window_width = window.bounds().size.width;
    let window_height = window.bounds().size.height;
    let max_width = (window_width * 0.8).min(px(600.0));
    let max_height = (window_height * 0.8).min(px(400.0));

    cx.new(move |_cx| ImageTooltipView {
        image_path,
        max_width: max_width.into(),
        max_height: max_height.into(),
    })
    .into()
}

struct ImageTooltipView {
    image_path: String,
    max_width: f32,
    max_height: f32,
}

impl Render for ImageTooltipView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let path = PathBuf::from(&self.image_path);
        let file_stem = path.file_stem().unwrap_or_default().to_string_lossy();
        let thumb_name = format!("{file_stem}.png");
        let thumb_path = path.parent().unwrap_or(&path).join(thumb_name);

        // Use thumbnail if exists, otherwise fallback to original
        let display_path = if thumb_path.exists() {
            thumb_path
        } else {
            path
        };

        div()
            .flex()
            .flex_col()
            .bg(cx.theme().popover)
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .shadow_lg()
            .p_2()
            .max_w(px(self.max_width))
            .max_h(px(self.max_height))
            .child(
                img(display_path)
                    .max_w(px(self.max_width - 16.0))
                    .max_h(px(self.max_height - 16.0))
                    .object_fit(gpui::ObjectFit::Contain),
            )
    }
}

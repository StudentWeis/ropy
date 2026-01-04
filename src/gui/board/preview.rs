/// Custom tooltip preview implementation that supports automatic line wrapping
use gpui::{
    AnyView, App, AppContext, IntoElement, ParentElement, Render, Styled, Window, div, img, px,
};
use gpui_component::ActiveTheme;
use image::ImageReader;
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
    let max_width = (window_width * 0.9).min(px(600.0));
    let max_height = (window_height * 0.9).min(px(400.0));

    let (width, height) = calculate_image_size(&image_path, max_width, max_height);

    cx.new(move |_cx| ImageTooltipView {
        image_path,
        width,
        height,
    })
    .into()
}

fn calculate_image_size(
    path: &str,
    max_w: gpui::Pixels,
    max_h: gpui::Pixels,
) -> (gpui::Pixels, gpui::Pixels) {
    if let Ok(reader) = ImageReader::open(path).and_then(|r| r.with_guessed_format())
        && let Ok(dims) = reader.into_dimensions()
    {
        let w = dims.0 as f32;
        let h = dims.1 as f32;

        let width_ratio = Into::<f32>::into(max_w) / w;
        let height_ratio = Into::<f32>::into(max_h) / h;
        let scale = width_ratio.min(height_ratio).min(1.0);

        return (px(w * scale), px(h * scale));
    }
    (max_w, max_h)
}

struct ImageTooltipView {
    image_path: String,
    width: gpui::Pixels,
    height: gpui::Pixels,
}

impl Render for ImageTooltipView {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        div().flex().flex_row().min_w_0().child(
            div()
                .bg(cx.theme().popover)
                .border_1()
                .border_color(cx.theme().border)
                .rounded_md()
                .shadow_lg()
                .p_2()
                .child(
                    img(PathBuf::from(&self.image_path))
                        .w(self.width)
                        .h(self.height),
                ),
        )
    }
}

//! Custom tooltip views that wrap automatically and stay within the
//! window's bounds — GPUI's built-in tooltip helpers don't expose either.

use std::path::PathBuf;

use gpui::{
    AnyView, App, AppContext, IntoElement, ParentElement, Render, Styled, Window, div, img, px,
};
use gpui_component::ActiveTheme;
use image::ImageReader;

const TOOLTIP_HORIZONTAL_MARGIN_PX: f32 = 40.0;
const IMAGE_TOOLTIP_MAX_WIDTH_PX: f32 = 600.0;
const IMAGE_TOOLTIP_MAX_HEIGHT_PX: f32 = 400.0;

/// Build a text tooltip whose width is capped against the current window so
/// long lines wrap instead of being clipped.
pub(super) fn simple_tooltip(content: impl Into<String>, window: &Window, cx: &mut App) -> AnyView {
    let content = content.into();
    let window_width = window.bounds().size.width;
    let max_width = (window_width - px(TOOLTIP_HORIZONTAL_MARGIN_PX)).into();

    cx.new(move |_cx| TooltipView { content, max_width }).into()
}

struct TooltipView {
    content: String,
    max_width: f32,
}

impl Render for TooltipView {
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut gpui::Context<'_, Self>,
    ) -> impl IntoElement {
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

/// Build an image tooltip scaled to fit within the current window while
/// respecting an absolute maximum so previews stay readable on large screens.
pub(super) fn image_tooltip(
    image_path: impl Into<String>,
    window: &Window,
    cx: &mut App,
) -> AnyView {
    let image_path = image_path.into();
    let window_width = window.bounds().size.width;
    let window_height = window.bounds().size.height;
    let max_width = (window_width * 0.9).min(px(IMAGE_TOOLTIP_MAX_WIDTH_PX));
    let max_height = (window_height * 0.9).min(px(IMAGE_TOOLTIP_MAX_HEIGHT_PX));

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
    if let Ok(reader) = ImageReader::open(path).and_then(ImageReader::with_guessed_format)
        && let Ok(dims) = reader.into_dimensions()
    {
        #[expect(
            clippy::cast_precision_loss,
            reason = "image dimensions in pixels are well below the f32 mantissa range"
        )]
        let (w, h) = (dims.0 as f32, dims.1 as f32);

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
    fn render(
        &mut self,
        _window: &mut Window,
        cx: &mut gpui::Context<'_, Self>,
    ) -> impl IntoElement {
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

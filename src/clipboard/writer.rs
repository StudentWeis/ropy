use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use gpui::{App, AppContext as _};
use image::ImageReader;

use super::CopyRequest;

/// Spawn the single long-lived task that owns the OS [`ClipboardContext`].
/// All copy operations funnel through the returned channel so we don't pay
/// the cost of recreating the context (and re-acquiring platform handles)
/// on every write.
pub fn start_clipboard_writer(cx: &App) -> async_channel::Sender<CopyRequest> {
    let (tx, rx) = async_channel::unbounded();

    cx.background_spawn(async move {
        let ctx = match ClipboardContext::new() {
            Ok(ctx) => ctx,
            Err(e) => {
                tracing::error!(error = %e, "failed to create clipboard output context");
                return;
            }
        };
        while let Ok(req) = rx.recv().await {
            match req {
                CopyRequest::Text { text, completion } => {
                    set_text(&ctx, text);
                    notify_completion(completion);
                }
                CopyRequest::Image { path, completion } => {
                    set_image(&ctx, &path);
                    notify_completion(completion);
                }
                CopyRequest::Files { paths, completion } => {
                    set_files(&ctx, &paths);
                    notify_completion(completion);
                }
                CopyRequest::RichText {
                    plain_text,
                    html,
                    rtf,
                    completion,
                } => {
                    set_rich_text(&ctx, plain_text, html, rtf);
                    notify_completion(completion);
                }
            }
        }
    })
    .detach();
    tx
}

fn load_image_from_path(path: &str) -> image::ImageResult<image::DynamicImage> {
    ImageReader::open(path)
        .map_err(image::ImageError::from)
        .and_then(image::ImageReader::decode)
}

fn set_text(ctx: &ClipboardContext, text: String) {
    if let Err(e) = ctx.set_text(text) {
        tracing::warn!(error = %e, "failed to set text to clipboard");
    }
}

fn set_image(ctx: &ClipboardContext, path: &str) {
    let img_res = load_image_from_path(path);
    if let Ok(img) = img_res {
        #[cfg(target_os = "macos")]
        {
            // `clipboard-rs::set_image` on macOS clears the pasteboard
            // before encoding, so for large images the listener can fire
            // on the empty intermediate state. Pre-encode here and use
            // `set_buffer` to keep clear→write atomic from its POV.
            let mut bytes = Vec::new();
            if img
                .write_to(
                    &mut std::io::Cursor::new(&mut bytes),
                    image::ImageFormat::Png,
                )
                .is_ok()
                && let Err(e) = ctx.set_buffer("public.png", bytes)
            {
                tracing::warn!(error = %e, "failed to set image to clipboard");
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            use clipboard_rs::common::RustImage;

            let rust_image = clipboard_rs::RustImageData::from_dynamic_image(img);
            if let Err(e) = ctx.set_image(rust_image) {
                tracing::warn!(error = %e, "failed to set image to clipboard");
            }
        }
    }
}

fn set_files(ctx: &ClipboardContext, paths: &[String]) {
    if paths.is_empty() {
        return;
    }

    let contents = vec![
        ClipboardContent::Text(paths.join("\n")),
        ClipboardContent::Files(paths.to_vec()),
    ];

    if let Err(error) = ctx.set(contents)
        && let Err(fallback_error) = ctx.set_files(paths.to_vec())
    {
        tracing::warn!(
            error = %error,
            fallback_error = %fallback_error,
            "failed to set files to clipboard"
        );
    }
}

fn set_rich_text(
    ctx: &ClipboardContext,
    plain_text: String,
    html: Option<String>,
    rtf: Option<String>,
) {
    let mut contents = vec![ClipboardContent::Text(plain_text.clone())];
    if let Some(html_content) = html {
        contents.push(ClipboardContent::Html(html_content));
    }
    if let Some(rtf_content) = rtf {
        contents.push(ClipboardContent::Rtf(rtf_content));
    }

    if let Err(error) = ctx.set(contents)
        && let Err(fallback_error) = ctx.set_text(plain_text)
    {
        tracing::warn!(
            error = %error,
            fallback_error = %fallback_error,
            "failed to set rich text to clipboard"
        );
    }
}

fn notify_completion(completion: Option<std::sync::mpsc::Sender<()>>) {
    if let Some(tx) = completion {
        let _ = tx.send(());
    }
}

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, time::Duration};

    use super::*;

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_load_image_from_path_when_png_exists_returns_decoded_image() {
        let temp_dir = tempfile::tempdir().unwrap();
        let image_path = temp_dir.path().join("test.png");
        image::DynamicImage::new_rgba8(2, 3)
            .save(&image_path)
            .unwrap();

        let image = load_image_from_path(image_path.to_str().unwrap()).unwrap();

        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 3);
    }

    #[test]
    fn test_load_image_from_path_when_file_is_missing_returns_error() {
        let result = load_image_from_path("/definitely/missing/image.png");

        assert!(result.is_err());
    }

    #[test]
    fn test_notify_completion_when_sender_exists_sends_signal() {
        let (completion_tx, completion_rx) = mpsc::channel();

        notify_completion(Some(completion_tx));

        assert_eq!(completion_rx.recv_timeout(Duration::from_secs(1)), Ok(()));
    }

    #[test]
    fn test_notify_completion_when_sender_missing_returns_without_panic() {
        notify_completion(None);
    }
}

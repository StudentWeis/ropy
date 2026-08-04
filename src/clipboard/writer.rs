use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use gpui::{App, AppContext as _};
use image::ImageReader;

use super::{ClipboardWriteError, ClipboardWriteResult, CopyRequest};

/// Spawn the single long-lived task that owns the OS [`ClipboardContext`].
/// All copy operations funnel through the returned channel so we don't pay
/// the cost of recreating the context (and re-acquiring platform handles)
/// on every write.
pub(crate) fn start_clipboard_writer(cx: &App) -> async_channel::Sender<CopyRequest> {
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
                    notify_completion(completion, set_text(&ctx, text));
                }
                CopyRequest::Image { path, completion } => {
                    notify_completion(completion, set_image(&ctx, &path));
                }
                CopyRequest::Files { paths, completion } => {
                    notify_completion(completion, set_files(&ctx, &paths));
                }
                CopyRequest::RichText {
                    plain_text,
                    html,
                    rtf,
                    completion,
                } => {
                    notify_completion(completion, set_rich_text(&ctx, plain_text, html, rtf));
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
        .and_then(ImageReader::decode)
}

fn clipboard_error(error: impl std::fmt::Display) -> ClipboardWriteError {
    ClipboardWriteError::Clipboard(error.to_string())
}

fn set_text(ctx: &ClipboardContext, text: String) -> ClipboardWriteResult {
    ctx.set_text(text).map_err(clipboard_error)
}

fn load_and_set_image(
    path: &str,
    set_image: impl FnOnce(image::DynamicImage) -> ClipboardWriteResult,
) -> ClipboardWriteResult {
    let image = load_image_from_path(path)?;
    set_image(image)
}

fn set_image(ctx: &ClipboardContext, path: &str) -> ClipboardWriteResult {
    load_and_set_image(path, |img| {
        #[cfg(target_os = "macos")]
        {
            // `clipboard-rs::set_image` on macOS clears the pasteboard
            // before encoding, so for large images the listener can fire
            // on the empty intermediate state. Pre-encode here and use
            // `set_buffer` to keep clear→write atomic from its POV.
            let mut bytes = Vec::new();
            img.write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )?;
            ctx.set_buffer("public.png", bytes).map_err(clipboard_error)
        }

        #[cfg(not(target_os = "macos"))]
        {
            use clipboard_rs::common::RustImage;

            let rust_image = clipboard_rs::RustImageData::from_dynamic_image(img);
            ctx.set_image(rust_image).map_err(clipboard_error)
        }
    })
}

fn set_files(ctx: &ClipboardContext, paths: &[String]) -> ClipboardWriteResult {
    if paths.is_empty() {
        return Err(ClipboardWriteError::EmptyFileList);
    }

    let contents = vec![
        ClipboardContent::Text(paths.join("\n")),
        ClipboardContent::Files(paths.to_vec()),
    ];

    if ctx.set(contents).is_ok() {
        return Ok(());
    }
    ctx.set_files(paths.to_vec()).map_err(clipboard_error)
}

fn set_rich_text(
    ctx: &ClipboardContext,
    plain_text: String,
    html: Option<String>,
    rtf: Option<String>,
) -> ClipboardWriteResult {
    let mut contents = vec![ClipboardContent::Text(plain_text.clone())];
    if let Some(html_content) = html {
        contents.push(ClipboardContent::Html(html_content));
    }
    if let Some(rtf_content) = rtf {
        contents.push(ClipboardContent::Rtf(rtf_content));
    }

    if ctx.set(contents).is_ok() {
        return Ok(());
    }
    ctx.set_text(plain_text).map_err(clipboard_error)
}

fn notify_completion(
    completion: Option<std::sync::mpsc::Sender<ClipboardWriteResult>>,
    result: ClipboardWriteResult,
) {
    if let Err(error) = &result {
        tracing::warn!(error = %error, "failed to write clipboard content");
    }
    if let Some(tx) = completion {
        let _ = tx.send(result);
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
    fn test_set_image_when_file_is_missing_returns_error() {
        let setter_called = std::cell::Cell::new(false);
        let result = load_and_set_image("/definitely/missing/image.png", |_| {
            setter_called.set(true);
            Ok(())
        });

        assert!(matches!(result, Err(ClipboardWriteError::Image(_))));
        assert!(!setter_called.get());
    }

    #[test]
    fn test_notify_completion_when_sender_exists_sends_result() {
        let (completion_tx, completion_rx) = mpsc::channel();

        notify_completion(Some(completion_tx), Err(ClipboardWriteError::EmptyFileList));

        assert!(matches!(
            completion_rx.recv_timeout(Duration::from_secs(1)),
            Ok(Err(ClipboardWriteError::EmptyFileList))
        ));
    }

    #[test]
    fn test_notify_completion_when_sender_missing_returns_without_panic() {
        notify_completion(None, Ok(()));
    }
}

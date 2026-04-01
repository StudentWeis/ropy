use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use gpui::{App, AppContext as _};
use image::ImageReader;

use super::CopyRequest;

/// Start a background task to handle clipboard write requests.
/// This avoids creating a new `ClipboardContext` and spawning a new task for each write.
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

/// Set text to clipboard
fn set_text(ctx: &ClipboardContext, text: String) {
    let _ = ctx.set_text(text);
}

/// Set image to clipboard. The image is read from the given file path.
/// After setting the image, the original file and its thumbnail are deleted.
fn set_image(ctx: &ClipboardContext, path: &str) {
    let img_res = load_image_from_path(path);
    if let Ok(img) = img_res {
        #[cfg(target_os = "macos")]
        {
            // On macOS, `clipboard-rs`'s default `set_image` implementation clears the clipboard,
            // then encodes the image to PNG, and finally writes it.
            // For large images, the encoding step takes time, creating a race condition where
            // the listener detects the "clear" event but fails to read the data because it's not written yet.
            //
            // To fix this, we pre-encode the image to PNG in memory and use `set_buffer` to write it.
            // This minimizes the time window between clearing and writing, ensuring the listener
            // finds the data when it reacts to the change event.
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

        // Platforms other than macOS can use RustImageData directly
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
    #[allow(clippy::unwrap_used)]
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
    #[allow(clippy::unwrap_used)]
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

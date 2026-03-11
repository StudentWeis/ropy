use clipboard_rs::{Clipboard, ClipboardContext};
use gpui::AsyncApp;
use image::ImageReader;

use super::CopyRequest;

/// Start a background task to handle clipboard write requests.
/// This avoids creating a new `ClipboardContext` and spawning a new task for each write.
pub fn start_clipboard_writer(async_app: &AsyncApp) -> async_channel::Sender<CopyRequest> {
    let (tx, rx) = async_channel::unbounded();
    let executor = async_app.background_executor();

    executor
        .spawn(async move {
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
                }
            }
        })
        .detach();
    tx
}

/// Set text to clipboard
fn set_text(ctx: &ClipboardContext, text: String) {
    let _ = ctx.set_text(text);
}

/// Set image to clipboard. The image is read from the given file path.
/// After setting the image, the original file and its thumbnail are deleted.
fn set_image(ctx: &ClipboardContext, path: &str) {
    let img_res = ImageReader::open(path)
        .map_err(image::ImageError::from)
        .and_then(image::ImageReader::decode);
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

fn notify_completion(completion: Option<std::sync::mpsc::Sender<()>>) {
    if let Some(tx) = completion {
        let _ = tx.send(());
    }
}

use super::CopyRequest;
use clipboard_rs::{Clipboard, ClipboardContext};
use image::ImageReader;
use std::sync::mpsc::{self, Sender};
use std::{fs, thread};

/// Start a background thread to handle clipboard write requests.
/// This avoids creating a new ClipboardContext and spawning a new thread for each write.
pub fn start_clipboard_writer() -> Sender<CopyRequest> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let ctx = ClipboardContext::new().unwrap();
        while let Ok(req) = rx.recv() {
            match req {
                CopyRequest::Text(text) => {
                    set_text(&ctx, text);
                }
                CopyRequest::Image(path) => {
                    set_image(&ctx, path);
                }
            }
        }
    });
    tx
}

/// Set text to clipboard
fn set_text(ctx: &ClipboardContext, text: String) {
    let _ = ctx.set_text(text);
}

/// Set image to clipboard. The image is read from the given file path.
/// After setting the image, the original file and its thumbnail are deleted.
fn set_image(ctx: &ClipboardContext, path: String) {
    let img_res = ImageReader::open(&path)
        .map_err(image::ImageError::from)
        .and_then(|r| r.decode());
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
                eprintln!("Failed to set image to clipboard: {}", e);
            }
        }

        // Platforms other than macOS can use RustImageData directly
        #[cfg(not(target_os = "macos"))]
        {
            let rust_image = clipboard_rs::RustImageData::from_dynamic_image(img);
            if let Err(e) = ctx.set_image(rust_image) {
                eprintln!("Failed to set image to clipboard: {}", e);
            }
        }

        // Delete original image file and thumbnail
        let _ = fs::remove_file(&path);
        let thumb_path = path.replace(".png", "_thumb.png");
        let _ = fs::remove_file(thumb_path);
    }
}

use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, RustImageData};
use image::ImageReader;

/// Copy text to system clipboard.
///
/// The function creates a local `ClipboardContext` for each call. This is a simple
/// cross-platform approach and avoids pinning contexts to threads; it should be
/// enough for the app use case (occasional copy on user click).
///
/// Returns `Ok(())` on success or `Err(String)` with a message describing failure.
pub fn set_text(text: &str) -> Result<(), String> {
    let ctx = ClipboardContext::new().unwrap();
    ctx.set_text(text.to_string())
        .map_err(|e| format!("set clipboard text: {}", e))
}

/// Copy image to system clipboard from file path.
pub fn set_image(path: &str) -> Result<(), String> {
    let ctx = ClipboardContext::new().unwrap();
    let img = ImageReader::open(path)
        .map_err(|e| format!("open image file: {}", e))?
        .decode()
        .map_err(|e| format!("decode image: {}", e))?;

    let rust_image = RustImageData::from_dynamic_image(img);
    ctx.set_image(rust_image)
        .map_err(|e| format!("set clipboard image: {}", e))
}

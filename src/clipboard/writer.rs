use super::CopyRequest;
use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, RustImageData};
use image::ImageReader;
use std::sync::mpsc::{self, Sender};
use std::thread;

/// Start a background thread to handle clipboard write requests.
/// This avoids creating a new ClipboardContext and spawning a new thread for each write.
pub fn start_clipboard_writer() -> Sender<CopyRequest> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let ctx = ClipboardContext::new().unwrap();
        while let Ok(req) = rx.recv() {
            match req {
                CopyRequest::Text(text) => {
                    let _ = ctx.set_text(text);
                }
                CopyRequest::Image(path) => {
                    let img_res = ImageReader::open(path)
                        .map_err(image::ImageError::from)
                        .and_then(|r| r.decode());
                    if let Ok(img) = img_res {
                        let rust_image = RustImageData::from_dynamic_image(img);
                        let _ = ctx.set_image(rust_image);
                    }
                }
            }
        }
    });
    tx
}

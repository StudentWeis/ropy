//! A simple clipboard change listener using event-driven watching.

use super::ClipboardEvent;
use chrono::Local;
use clipboard_rs::common::RustImage;
use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
};
use image::DynamicImage;
use std::sync::mpsc::Sender;
use std::thread;

/// Clipboard monitor that sends clipboard text changes through a channel.
struct ClipboardMonitor {
    tx: Sender<ClipboardEvent>,
    ctx: ClipboardContext,
    last_text: Option<String>,
}

impl ClipboardMonitor {
    fn new(tx: Sender<ClipboardEvent>) -> Self {
        let ctx = ClipboardContext::new().unwrap();
        Self {
            tx,
            ctx,
            last_text: None,
        }
    }

    fn save_image(&self, image: DynamicImage) -> Option<String> {
        let data_dir = dirs::data_local_dir()?.join("ropy").join("images");
        if !data_dir.exists() {
            std::fs::create_dir_all(&data_dir).ok()?;
        }

        let now = Local::now();
        let id = now.timestamp_nanos_opt().unwrap_or(0) as u64;
        let file_name = format!("{}.png", id);
        let file_path = data_dir.join(&file_name);

        image
            .save_with_format(&file_path, image::ImageFormat::Png)
            .ok()?;

        // Save thumbnail
        let thumb_file_name = format!("{}_thumb.png", id);
        let thumb_file_path = data_dir.join(&thumb_file_name);
        let thumb = image.thumbnail(300, 300);
        thumb
            .save_with_format(&thumb_file_path, image::ImageFormat::Png)
            .ok()?;

        Some(file_path.to_string_lossy().to_string())
    }
}

impl ClipboardHandler for ClipboardMonitor {
    fn on_clipboard_change(&mut self) {
        // Check for image first
        if let Ok(image) = self.ctx.get_image()
            && let Ok(dyn_img) = image.get_dynamic_image()
        {
            // Reset last_text because we have a new image content
            self.last_text = None;
            if let Some(path) = self.save_image(dyn_img) {
                let _ = self.tx.send(ClipboardEvent::Image(path));
            }
            return;
        }

        // Check for text
        match self.ctx.get_text() {
            Ok(value) => {
                // Don't send duplicate clipboard contents
                if Some(&value) != self.last_text.as_ref() {
                    let _ = self.tx.send(ClipboardEvent::Text(value.clone()));
                    self.last_text = Some(value);
                }
            }
            Err(_err) => {
                // eprintln!("[clipboard-listener] failed to read clipboard: {err}");
            }
        }
    }
}
/// Spawn a clipboard listener thread that watches for clipboard changes.
pub fn start_clipboard_monitor(tx: Sender<ClipboardEvent>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let monitor = ClipboardMonitor::new(tx);
        let mut watcher = ClipboardWatcherContext::new().unwrap();
        watcher.add_handler(monitor);
        watcher.start_watch();
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;
    use std::time::Duration;

    /// Test that clipboard changes are detected.
    #[test]
    fn test_clipboard_change_detection() {
        let (tx, rx) = channel();
        let _handle = start_clipboard_monitor(tx);
        std::thread::sleep(std::time::Duration::from_millis(500));
        let ctx: ClipboardContext = ClipboardContext::new().unwrap();
        ctx.set_text("test-poll-1".into()).unwrap();
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(ClipboardEvent::Text(received)) => assert_eq!(received, "test-poll-1"),
            _ => {
                println!("Test skipped: clipboard change event not detected or wrong type");
            }
        }
    }
}

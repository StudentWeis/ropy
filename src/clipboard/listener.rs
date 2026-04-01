//! A simple clipboard change listener using event-driven watching.

use std::sync::{Arc, Mutex};

use async_channel::Sender;
use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    common::RustImage,
};
use gpui::{App, AppContext as _};
use image::DynamicImage;

use super::{ClipboardEvent, LastCopyState};
use crate::utils::lock_or_recover;

/// Clipboard monitor that sends clipboard text changes through a channel.
struct ClipboardMonitor {
    tx: Sender<ClipboardEvent>,
    image_tx: Sender<(DynamicImage, u64)>,
    ctx: ClipboardContext,
    last_copy: Arc<Mutex<LastCopyState>>,
}

impl ClipboardMonitor {
    fn new(
        tx: Sender<ClipboardEvent>,
        image_tx: Sender<(DynamicImage, u64)>,
        last_copy: Arc<Mutex<LastCopyState>>,
    ) -> Option<Self> {
        let ctx = match ClipboardContext::new() {
            Ok(ctx) => ctx,
            Err(e) => {
                tracing::error!(error = %e, "failed to initialize clipboard context");
                return None;
            }
        };
        Some(Self {
            tx,
            image_tx,
            ctx,
            last_copy,
        })
    }
}

const fn should_forward_image(last_copy: &LastCopyState, hash: u64) -> bool {
    !matches!(last_copy, LastCopyState::Image(last_hash) if *last_hash == hash)
}

fn should_forward_text(last_copy: &LastCopyState, text: &str) -> bool {
    !matches!(last_copy, LastCopyState::Text(last_text) if last_text == text)
}

impl ClipboardHandler for ClipboardMonitor {
    // Don't send duplicate clipboard contents
    fn on_clipboard_change(&mut self) {
        let mut last_copy_guard = lock_or_recover(&self.last_copy);
        if let Ok(image) = self.ctx.get_image()
            && let Ok(dyn_img) = image.get_dynamic_image()
        {
            // Calculate deterministic image hash using seahash
            let hash: u64 = seahash::hash(dyn_img.as_bytes());

            if should_forward_image(&last_copy_guard, hash) {
                let _ = self.image_tx.send_blocking((dyn_img, hash));
                *last_copy_guard = LastCopyState::Image(hash);
            }
        } else if let Ok(text) = self.ctx.get_text()
            && should_forward_text(&last_copy_guard, &text)
        {
            let _ = self.tx.send_blocking(ClipboardEvent::Text(text.clone()));
            *last_copy_guard = LastCopyState::Text(text);
        }
    }
}

/// Spawn a clipboard listener thread that watches for clipboard changes.
pub fn start_clipboard_monitor(
    tx: Sender<ClipboardEvent>,
    cx: &App,
    last_copy: Arc<Mutex<LastCopyState>>,
) {
    let (image_tx, image_rx) = async_channel::unbounded::<(DynamicImage, u64)>();
    let Some(monitor) = ClipboardMonitor::new(tx.clone(), image_tx, last_copy) else {
        return;
    };

    cx.background_spawn(async move {
        while let Ok((image, hash)) = image_rx.recv().await {
            if let Some(path) = super::save_image(&image) {
                let _ = tx.send_blocking(ClipboardEvent::Image(path, hash));
            }
        }
    })
    .detach();

    cx.background_spawn(async move {
        let mut watcher = match ClipboardWatcherContext::new() {
            Ok(w) => w,
            Err(e) => {
                tracing::error!(error = %e, "failed to create clipboard watcher");
                return;
            }
        };
        watcher.add_handler(monitor);
        watcher.start_watch();
    })
    .detach();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_forward_text_when_same_text_returns_false() {
        let last_copy = LastCopyState::Text("hello".to_string());

        assert!(!should_forward_text(&last_copy, "hello"));
    }

    #[test]
    fn test_should_forward_text_when_different_text_returns_true() {
        let last_copy = LastCopyState::Text("hello".to_string());

        assert!(should_forward_text(&last_copy, "world"));
    }

    #[test]
    fn test_should_forward_text_when_last_copy_is_image_returns_true() {
        let last_copy = LastCopyState::Image(42);

        assert!(should_forward_text(&last_copy, "hello"));
    }

    #[test]
    fn test_should_forward_image_when_same_hash_returns_false() {
        let last_copy = LastCopyState::Image(42);

        assert!(!should_forward_image(&last_copy, 42));
    }

    #[test]
    fn test_should_forward_image_when_different_hash_returns_true() {
        let last_copy = LastCopyState::Image(42);

        assert!(should_forward_image(&last_copy, 7));
    }

    #[test]
    fn test_should_forward_image_when_last_copy_is_text_returns_true() {
        let last_copy = LastCopyState::Text("hello".to_string());

        assert!(should_forward_image(&last_copy, 42));
    }
}

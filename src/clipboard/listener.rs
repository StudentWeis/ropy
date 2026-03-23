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

impl ClipboardHandler for ClipboardMonitor {
    // Don't send duplicate clipboard contents
    fn on_clipboard_change(&mut self) {
        let mut last_copy_guard = match self.last_copy.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Ok(image) = self.ctx.get_image()
            && let Ok(dyn_img) = image.get_dynamic_image()
        {
            // Calculate deterministic image hash using seahash
            let hash: u64 = seahash::hash(dyn_img.as_bytes());

            if !matches!(*last_copy_guard, LastCopyState::Image(h) if h == hash) {
                let _ = self.image_tx.send_blocking((dyn_img, hash));
                *last_copy_guard = LastCopyState::Image(hash);
            }
        } else if let Ok(text) = self.ctx.get_text()
            && !matches!(*last_copy_guard, LastCopyState::Text(ref last_text) if *last_text == text)
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

//! A simple clipboard change listener using event-driven watching.

use std::sync::{Arc, Mutex, RwLock};

use async_channel::Sender;
use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    common::RustImage,
};
use gpui::{AsyncApp, WindowHandle};
use gpui_component::Root;
use image::DynamicImage;

use super::{ClipboardEvent, LastCopyState};
use crate::{
    config::Settings,
    repository::{ClipboardRecord, ClipboardRepository},
};

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
    async_app: &AsyncApp,
    last_copy: Arc<Mutex<LastCopyState>>,
) {
    let (image_tx, image_rx) = async_channel::unbounded::<(DynamicImage, u64)>();
    let Some(monitor) = ClipboardMonitor::new(tx.clone(), image_tx, last_copy) else {
        return;
    };
    let executor = async_app.background_executor();

    executor
        .spawn(async move {
            while let Ok((image, hash)) = image_rx.recv().await {
                if let Some(path) = super::save_image(&image) {
                    let _ = tx.send_blocking(ClipboardEvent::Image(path, hash));
                }
            }
        })
        .detach();

    executor
        .spawn(async move {
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

pub fn start_clipboard_listener(
    clipboard_rx: async_channel::Receiver<ClipboardEvent>,
    shared_records: Arc<Mutex<Vec<ClipboardRecord>>>,
    repository: Option<Arc<ClipboardRepository>>,
    settings: Arc<RwLock<Settings>>,
    async_app: AsyncApp,
    window_handle: WindowHandle<Root>,
) {
    let (notify_tx, notify_rx) = async_channel::unbounded::<()>();
    let bg_executor = async_app.background_executor().clone();
    let fg_executor = async_app.foreground_executor().clone();

    bg_executor
        .spawn(async move {
            while let Ok(event) = clipboard_rx.recv().await {
                if let Some(ref repo) = repository {
                    let result = match event {
                        ClipboardEvent::Text(text) => repo.save_text(text),
                        ClipboardEvent::Image(path, hash) => repo.save_image_from_path(path, hash),
                    };

                    match result {
                        Ok(record) => {
                            {
                                let (max_display, max_storage) = {
                                    let settings_guard = match settings.read() {
                                        Ok(g) => g,
                                        Err(e) => e.into_inner(),
                                    };
                                    (
                                        settings_guard.storage.max_history_records,
                                        settings_guard.storage.max_storage_records,
                                    )
                                };
                                let mut guard = match shared_records.lock() {
                                    Ok(g) => g,
                                    Err(poisoned) => poisoned.into_inner(),
                                };
                                // Remove existing record with same id (dedup upsert)
                                guard.retain(|r| r.id != record.id);
                                guard.insert(0, record);
                                // Truncate in-memory records to display limit
                                if guard.len() > max_display {
                                    guard.truncate(max_display);
                                }
                                drop(guard);
                                // Cleanup repository to storage limit
                                repo.cleanup_old_records(max_storage).ok();
                            }
                            let _ = notify_tx.send(()).await;
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "failed to save clipboard record");
                        }
                    }
                }
            }
        })
        .detach();

    // Notify GUI to refresh clipboard history
    fg_executor
        .spawn(async move {
            while (notify_rx.recv().await).is_ok() {
                let _ = async_app.update(|cx| {
                    window_handle
                        .update(cx, |_, _, cx| {
                            cx.notify();
                        })
                        .ok();
                });
            }
        })
        .detach();
}

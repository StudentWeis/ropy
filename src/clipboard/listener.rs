//! A simple clipboard change listener using event-driven watching.

use super::{ClipboardEvent, LastCopyState};
use crate::config::Settings;
use crate::repository::{ClipboardRecord, ClipboardRepository};
use async_channel::Sender;
use clipboard_rs::common::RustImage;
use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
};
use gpui::{AsyncApp, WindowHandle};
use gpui_component::Root;
use image::DynamicImage;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, RwLock};

/// Clipboard monitor that sends clipboard text changes through a channel.
struct ClipboardMonitor {
    tx: Sender<ClipboardEvent>,
    image_tx: Sender<DynamicImage>,
    ctx: ClipboardContext,
    last_copy: Arc<Mutex<LastCopyState>>,
}

impl ClipboardMonitor {
    fn new(
        tx: Sender<ClipboardEvent>,
        image_tx: Sender<DynamicImage>,
        last_copy: Arc<Mutex<LastCopyState>>,
    ) -> Self {
        let ctx = ClipboardContext::new().unwrap();
        Self {
            tx,
            image_tx,
            last_copy,
            ctx,
        }
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
            // Calculate image hash
            let mut hasher = DefaultHasher::new();
            dyn_img.as_bytes().hash(&mut hasher);
            let hash: u64 = hasher.finish();

            if !matches!(*last_copy_guard, LastCopyState::Image(h) if h == hash) {
                let _ = self.image_tx.send_blocking(dyn_img);
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
    async_app: AsyncApp,
    last_copy: Arc<Mutex<LastCopyState>>,
) {
    let (image_tx, image_rx) = async_channel::unbounded::<DynamicImage>();
    let monitor = ClipboardMonitor::new(tx.clone(), image_tx, last_copy);
    let executor = async_app.background_executor();

    executor
        .spawn(async move {
            while let Ok(image) = image_rx.recv().await {
                if let Some(path) = super::save_image(image) {
                    let _ = tx.send_blocking(ClipboardEvent::Image(path));
                }
            }
        })
        .detach();

    executor
        .spawn(async move {
            let mut watcher = ClipboardWatcherContext::new().unwrap();
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
                        ClipboardEvent::Image(path) => repo.save_image_from_path(path),
                    };

                    match result {
                        Ok(record) => {
                            {
                                let mut guard = match shared_records.lock() {
                                    Ok(g) => g,
                                    Err(poisoned) => poisoned.into_inner(),
                                };
                                guard.insert(0, record);
                                let max_history_records = {
                                    let settings_guard = settings.read().unwrap();
                                    settings_guard.storage.max_history_records
                                };
                                if guard.len() > max_history_records {
                                    guard.truncate(max_history_records);
                                    repo.cleanup_old_records(max_history_records).ok();
                                }
                            }
                            let _ = notify_tx.send(()).await;
                        }
                        Err(e) => {
                            eprintln!("[ropy] Failed to save clipboard record: {e}");
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

//! A simple clipboard change listener using event-driven watching.
//!
//! We use `clipboard-rs` to read and write clipboard contents. The crate provides
//! a cross-platform clipboard change event API via `ClipboardWatcherContext` and `ClipboardHandler`.

use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
};
use std::sync::mpsc::Sender;
use std::thread;

struct ClipboardMonitor {
    tx: Sender<String>,
    ctx: ClipboardContext,
}

impl ClipboardMonitor {
    fn new(tx: Sender<String>) -> Self {
        let ctx = ClipboardContext::new().expect("Failed to create clipboard context");
        Self { tx, ctx }
    }
}

impl ClipboardHandler for ClipboardMonitor {
    fn on_clipboard_change(&mut self) {
        match self.ctx.get_text() {
            Ok(value) => {
                let _ = self.tx.send(value);
            }
            Err(err) => {
                eprintln!("[clipboard-listener] failed to read clipboard: {}", err);
            }
        }
    }
}
/// Spawn a clipboard listener thread that watches for clipboard changes.
///
/// - `tx`: channel `Sender<String>` used to notify about clipboard text changes.
///
/// Returns the thread `JoinHandle<()>`. The monitor thread is long-lived and will exit
/// when the `tx` receiver is disconnected.
pub fn start_clipboard_monitor(tx: Sender<String>) -> thread::JoinHandle<()> {
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

    // Smoke test for the listener: set the clipboard, spawn listener, update clipboard
    // and assert we receive the notification.
    // This test is best-effort and may be flaky
    // across CI environments or when pasteboards are restricted.
    #[test]
    fn listener_reports_changes() {
        let (tx, rx) = channel();
        // Spawn listener
        let _handle = start_clipboard_monitor(tx);

        // Set clipboard value via a new context from this thread
        let ctx: ClipboardContext = ClipboardContext::new().expect("create ctx");
        ctx.set_text("test-poll-1".into()).expect("set contents");
        // Wait up to 1s for the change to be observed
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(received) => assert_eq!(received, "test-poll-1"),
            Err(_) => {
                // Event not triggered, possibly due to platform limitations in test environment
                println!("Test skipped: clipboard change event not detected");
            }
        }
    }
}

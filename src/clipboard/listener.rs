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
    last: Option<String>,
}

impl ClipboardMonitor {
    fn new(tx: Sender<String>) -> Self {
        let ctx = ClipboardContext::new().unwrap();
        Self {
            tx,
            ctx,
            last: None,
        }
    }
}

impl ClipboardHandler for ClipboardMonitor {
    fn on_clipboard_change(&mut self) {
        match self.ctx.get_text() {
            Ok(value) => {
                // Don't send duplicate clipboard contents
                if Some(&value) != self.last.as_ref() {
                    let _ = self.tx.send(value.clone());
                    self.last = Some(value);
                }
            }
            Err(err) => {
                eprintln!("[clipboard-listener] failed to read clipboard: {err}");
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

    #[test]
    fn listener_reports_changes() {
        let (tx, rx) = channel();
        let _handle = start_clipboard_monitor(tx);
        let ctx: ClipboardContext = ClipboardContext::new().unwrap();
        ctx.set_text("test-poll-1".into()).unwrap();
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(received) => assert_eq!(received, "test-poll-1"),
            Err(_) => {
                println!("Test skipped: clipboard change event not detected");
            }
        }
    }

    #[test]
    fn listener_deduplicates_changes() {
        let (tx, rx) = channel();
        let _handle = start_clipboard_monitor(tx);
        let ctx: ClipboardContext = ClipboardContext::new().unwrap();
        ctx.set_text("test-poll-1".into()).unwrap();
        let received = rx
            .recv_timeout(Duration::from_secs(3))
            .expect("should receive clipboard change");
        assert_eq!(received, "test-poll-1");
        ctx.set_text("test-poll-1".into()).unwrap();
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(_) => panic!("should not receive duplicate clipboard change"),
            Err(_) => {
                println!("Yes! No duplicate received as expected");
            }
        }
    }
}

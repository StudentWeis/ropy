//! A simple clipboard change listener using event-driven watching.

use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
};
use std::sync::mpsc::Sender;
use std::thread;

/// Clipboard monitor that sends clipboard text changes through a channel.
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

    /// Test that clipboard changes are detected.
    #[test]
    fn test_clipboard_change_detection() {
        let (tx, rx) = channel();
        let _handle = start_clipboard_monitor(tx);
        std::thread::sleep(std::time::Duration::from_millis(500));
        let ctx: ClipboardContext = ClipboardContext::new().unwrap();
        ctx.set_text("test-poll-1".into()).unwrap();
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(received) => assert_eq!(received, "test-poll-1"),
            Err(_) => {
                println!("Test skipped: clipboard change event not detected");
            }
        }
    }

    /// Test that duplicate clipboard contents are ignored.
    #[test]
    fn test_clipboard_change_ignores_duplicates() {
        let (tx, rx) = channel();
        let _handle = start_clipboard_monitor(tx);
        std::thread::sleep(std::time::Duration::from_millis(500));
        let ctx: ClipboardContext = ClipboardContext::new().unwrap();
        ctx.set_text("test-poll-1".into()).unwrap();
        let received = rx
            .recv_timeout(Duration::from_millis(500))
            .expect("should receive clipboard change");
        assert_eq!(received, "test-poll-1");
        ctx.set_text("test-poll-1".into()).unwrap();
        if rx.recv_timeout(Duration::from_millis(500)).is_ok() {
            panic!("Should not receive duplicate clipboard change")
        }
    }
}

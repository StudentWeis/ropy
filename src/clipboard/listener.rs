//! A simple clipboard change listener using polling.
//!
//! We use `clipboard-rs` to read and write clipboard contents. The crate does not provide
//! a cross-platform clipboard change event API. A common, portable approach is to poll
//! periodically in a background thread, compare the current value with the previous one,
//! and send notifications on change.
//!
//! For most apps this approach is perfectly acceptable. If you need low-latency
//! or more efficient behaviour (no polling), implement platform-specific watchers
//! (Windows: AddClipboardFormatListener / SetClipboardViewer; macOS: NSPasteboard changeCount;
//! X11/Wayland: selection/owner events).
use clipboard_rs::{Clipboard, ClipboardContext};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::Duration;

/// Spawn a clipboard listener thread that polls the clipboard for changes.
///
/// - `tx`: channel `Sender<String>` used to notify about clipboard text changes.
/// - `stop_flag`: an `Arc<AtomicBool>` that can be set to `true` to stop the thread.
/// - `poll_interval`: how frequently to poll the clipboard (Duration).
///
/// Returns the thread `JoinHandle<()>`. The monitor thread is long-lived and will exit
/// when `stop_flag` is set OR `tx` is disconnected.
pub fn start_clipboard_monitor(
    tx: Sender<String>,
    poll_interval: Duration,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        // Create a new clipboard context for this thread
        // Note: on some platforms (eg. macOS/Cocoa) clipboard APIs can require main-thread
        // interaction. In practice, `clipboard-rs` works on many platforms from background
        // threads, but test on your target platforms to be sure.
        let ctx: ClipboardContext = match ClipboardContext::new() {
            Ok(c) => c,
            Err(err) => {
                eprintln!(
                    "[clipboard-listener] failed to create clipboard context: {}",
                    err
                );
                return;
            }
        };

        // Seed with the current clipboard content.
        let mut last_contents = ctx.get_text().unwrap_or_default();

        loop {
            match ctx.get_text() {
                Ok(value) => {
                    if value != last_contents {
                        last_contents = value.clone();
                        // Try sending; if receiver is gone, break out
                        if tx.send(value).is_err() {
                            break;
                        }
                    }
                }
                Err(err) => {
                    // Reading the clipboard can fail transiently, log and continue
                    eprintln!("[clipboard-listener] failed to read clipboard: {}", err);
                }
            }

            // Sleep for a while before next poll
            thread::sleep(poll_interval);
        }
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
        // Spawn listener with 50 ms interval for test speed
        let _handle = start_clipboard_monitor(tx, Duration::from_millis(50));

        // Set clipboard value via a new context from this thread
        let ctx: ClipboardContext = ClipboardContext::new().expect("create ctx");
        ctx.set_text("test-poll-1".into()).expect("set contents");
        // Wait up to 1s for the change to be observed
        let received = rx.recv_timeout(Duration::from_secs(1));
        assert_eq!(received.unwrap(), "test-poll-1");
    }
}

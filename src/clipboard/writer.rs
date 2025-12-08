use clipboard_rs::{Clipboard, ClipboardContext};

/// Copy text to system clipboard.
///
/// The function creates a local `ClipboardContext` for each call. This is a simple
/// cross-platform approach and avoids pinning contexts to threads; it should be
/// enough for the app use case (occasional copy on user click).
///
/// Returns `Ok(())` on success or `Err(String)` with a message describing failure.
pub fn set_text(text: &str) -> Result<(), String> {
    let ctx = ClipboardContext::new().map_err(|e| format!("init clipboard ctx: {}", e))?;
    ctx.set_text(text.to_string())
        .map_err(|e| format!("set clipboard text: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clipboard_rs::ClipboardContext;

    #[test]
    fn writer_sets_clipboard_text() {
        use std::thread::sleep;
        use std::time::Duration;

        let v = "ropys_test_text_123";
        set_text(v).expect("set_text should succeed");
        // create a new context and read it back; the clipboard may take a short while to propagate
        let ctx: ClipboardContext = ClipboardContext::new().expect("create ctx");
        for _ in 0..20 {
            if let Ok(text) = ctx.get_text()
                && text == v
            {
                return;
            }
            sleep(Duration::from_millis(50));
        }
        panic!("clipboard writer test failed to observe set clipboard contents in time");
    }
}

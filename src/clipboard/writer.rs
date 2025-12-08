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

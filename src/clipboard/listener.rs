//! A simple clipboard change listener using event-driven watching.

use std::sync::{Arc, Mutex};

use async_channel::Sender;
use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    ContentFormat, common::RustImage,
};
use gpui::{App, AppContext as _};
use image::DynamicImage;

use super::{ClipboardEvent, LastCopyState};
use crate::utils::{hash_file_paths, lock_or_recover, normalize_file_paths};

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

const fn should_forward_files(last_copy: &LastCopyState, hash: u64) -> bool {
    !matches!(last_copy, LastCopyState::Files(last_hash) if *last_hash == hash)
}

enum ClipboardPayload {
    Files(Vec<String>),
    RichText {
        plain_text: String,
        html: Option<String>,
        rtf: Option<String>,
    },
    Image(DynamicImage),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardPayloadKind {
    Files,
    RichText,
    Image,
    Text,
}

fn preferred_clipboard_payload_kind(
    files: Option<ClipboardPayloadKind>,
    rich_text: Option<ClipboardPayloadKind>,
    image: Option<ClipboardPayloadKind>,
    text: Option<ClipboardPayloadKind>,
) -> Option<ClipboardPayloadKind> {
    files.or(rich_text).or(image).or(text)
}

fn detect_clipboard_payload(ctx: &ClipboardContext) -> Option<ClipboardPayload> {
    let files = ctx
        .get_files()
        .ok()
        .map(|paths| normalize_file_paths(&paths))
        .filter(|paths| !paths.is_empty());
    let text = files.is_none().then(|| ctx.get_text().ok()).flatten();
    let has_html = text.is_some() && ctx.has(ContentFormat::Html);
    let has_rtf = text.is_some() && ctx.has(ContentFormat::Rtf);
    let rich_text = text
        .as_ref()
        .filter(|_| has_html || has_rtf)
        .map(|plain_text| ClipboardPayload::RichText {
            plain_text: plain_text.clone(),
            html: has_html.then(|| ctx.get_html().ok()).flatten(),
            rtf: has_rtf.then(|| ctx.get_rich_text().ok()).flatten(),
        });
    let image = (files.is_none() && rich_text.is_none())
        .then(|| {
            ctx.get_image()
                .ok()
                .and_then(|image| image.get_dynamic_image().ok())
        })
        .flatten();

    match preferred_clipboard_payload_kind(
        files.as_ref().map(|_| ClipboardPayloadKind::Files),
        rich_text.as_ref().map(|_| ClipboardPayloadKind::RichText),
        image.as_ref().map(|_| ClipboardPayloadKind::Image),
        text.as_ref().map(|_| ClipboardPayloadKind::Text),
    ) {
        Some(ClipboardPayloadKind::Files) => files.map(ClipboardPayload::Files),
        Some(ClipboardPayloadKind::RichText) => rich_text,
        Some(ClipboardPayloadKind::Image) => image.map(ClipboardPayload::Image),
        Some(ClipboardPayloadKind::Text) => text.map(ClipboardPayload::Text),
        None => None,
    }
}

impl ClipboardHandler for ClipboardMonitor {
    // Don't send duplicate clipboard contents
    fn on_clipboard_change(&mut self) {
        let mut last_copy_guard = lock_or_recover(&self.last_copy);

        match detect_clipboard_payload(&self.ctx) {
            Some(ClipboardPayload::Files(files)) => {
                let hash = hash_file_paths(&files);
                if should_forward_files(&last_copy_guard, hash) {
                    if let Err(e) = self.tx.send_blocking(ClipboardEvent::Files(files)) {
                        tracing::warn!(error = %e, "failed to send files to clipboard event channel");
                    }
                    *last_copy_guard = LastCopyState::Files(hash);
                }
            }
            Some(ClipboardPayload::Image(dyn_img)) => {
                let hash: u64 = seahash::hash(dyn_img.as_bytes());

                if should_forward_image(&last_copy_guard, hash) {
                    if let Err(e) = self.image_tx.send_blocking((dyn_img, hash)) {
                        tracing::warn!(error = %e, "failed to send image to processing channel");
                    }
                    *last_copy_guard = LastCopyState::Image(hash);
                }
            }
            Some(ClipboardPayload::RichText {
                plain_text,
                html,
                rtf,
            }) if should_forward_text(&last_copy_guard, &plain_text) => {
                if let Err(e) = self.tx.send_blocking(ClipboardEvent::RichText {
                    plain_text: plain_text.clone(),
                    html,
                    rtf,
                }) {
                    tracing::warn!(error = %e, "failed to send rich text to clipboard event channel");
                }
                *last_copy_guard = LastCopyState::Text(plain_text);
            }
            Some(ClipboardPayload::Text(text)) if should_forward_text(&last_copy_guard, &text) => {
                if let Err(e) = self.tx.send_blocking(ClipboardEvent::Text(text.clone())) {
                    tracing::warn!(error = %e, "failed to send text to clipboard event channel");
                }
                *last_copy_guard = LastCopyState::Text(text);
            }
            _ => {}
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
            if let Some(path) = super::save_image(&image, hash)
                && let Err(e) = tx.send_blocking(ClipboardEvent::Image(path, hash))
            {
                tracing::warn!(error = %e, "failed to send image event to clipboard channel");
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
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(true, true, true, true, Some(ClipboardPayloadKind::Files))]
    #[case(true, false, false, true, Some(ClipboardPayloadKind::Files))]
    #[case(false, true, true, true, Some(ClipboardPayloadKind::RichText))]
    #[case(false, true, false, true, Some(ClipboardPayloadKind::RichText))]
    #[case(false, false, true, true, Some(ClipboardPayloadKind::Image))]
    #[case(false, false, false, true, Some(ClipboardPayloadKind::Text))]
    #[case(false, false, false, false, None)]
    fn test_preferred_clipboard_payload_kind_when_formats_present_returns_expected_priority(
        #[case] has_files: bool,
        #[case] has_rich_text: bool,
        #[case] has_image: bool,
        #[case] has_text: bool,
        #[case] expected: Option<ClipboardPayloadKind>,
    ) {
        assert_eq!(
            preferred_clipboard_payload_kind(
                has_files.then_some(ClipboardPayloadKind::Files),
                has_rich_text.then_some(ClipboardPayloadKind::RichText),
                has_image.then_some(ClipboardPayloadKind::Image),
                has_text.then_some(ClipboardPayloadKind::Text),
            ),
            expected
        );
    }

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
    fn test_should_forward_files_when_same_hash_returns_false() {
        let last_copy = LastCopyState::Files(42);

        assert!(!should_forward_files(&last_copy, 42));
    }

    #[test]
    fn test_should_forward_files_when_different_hash_returns_true() {
        let last_copy = LastCopyState::Files(42);

        assert!(should_forward_files(&last_copy, 7));
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

    #[test]
    fn test_should_forward_files_when_last_copy_is_text_returns_true() {
        let last_copy = LastCopyState::Text("hello".to_string());

        assert!(should_forward_files(&last_copy, 42));
    }
}

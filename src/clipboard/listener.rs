//! A simple clipboard change listener using event-driven watching.

use std::sync::{Arc, Mutex};

use async_channel::{Receiver, Sender};
use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    ContentFormat, common::RustImage,
};
use gpui::{App, AppContext as _};
use image::DynamicImage;

use super::{ClipboardEvent, LastCopyState};
use crate::utils::{hash_file_paths, lock_or_recover, normalize_file_paths};

/// Capacity for the image processing channel between the OS clipboard
/// callback and the image-encoding background task. Images are large
/// (`DynamicImage`), so the channel uses a single-slot, newest-wins overwrite
/// policy: when full, the producer drops the oldest queued image and inserts
/// the new one. This keeps memory bounded under bursty image copies while
/// preserving the most recent (and only meaningful) payload.
const IMAGE_PROCESSING_CHANNEL_CAPACITY: usize = 1;

/// Try to enqueue an image payload with newest-wins overwrite semantics for
/// the single-slot image processing channel.
///
/// On a full channel, the oldest queued image is drained via `image_drain` and
/// the new payload is then enqueued. Returns `Ok(())` if the new payload is
/// eventually enqueued, `Err(_)` only if the channel is closed.
fn try_send_image_overwrite(
    image_tx: &Sender<(DynamicImage, u64)>,
    image_drain: &Receiver<(DynamicImage, u64)>,
    payload: (DynamicImage, u64),
) -> Result<(), async_channel::TrySendError<(DynamicImage, u64)>> {
    match image_tx.try_send(payload) {
        Ok(()) => Ok(()),
        Err(async_channel::TrySendError::Full(payload)) => {
            // Discard the oldest queued image; a concurrent consumer may have
            // just drained it, in which case the slot is already free.
            let _ = image_drain.try_recv();
            image_tx.try_send(payload)
        }
        Err(closed) => Err(closed),
    }
}

/// Clipboard monitor that sends clipboard text changes through a channel.
struct ClipboardMonitor {
    tx: Sender<ClipboardEvent>,
    image_tx: Sender<(DynamicImage, u64)>,
    /// Producer-side handle on the image channel used solely to drop the
    /// oldest queued image when the single-slot channel is full
    /// (newest-wins overwrite policy). Cloned from the same channel as
    /// `image_tx`, so this neither prevents the consumer task from receiving
    /// nor counts as an additional consumer of meaningful events.
    image_drain: Receiver<(DynamicImage, u64)>,
    ctx: ClipboardContext,
    last_copy: Arc<Mutex<LastCopyState>>,
}

impl ClipboardMonitor {
    fn new(
        tx: Sender<ClipboardEvent>,
        image_tx: Sender<(DynamicImage, u64)>,
        image_drain: Receiver<(DynamicImage, u64)>,
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
            image_drain,
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
    // Don't send duplicate clipboard contents.
    //
    // Important: `LastCopyState` is advanced only after a successful enqueue.
    // Updating it after a dropped/Full event would let the dedup gate filter
    // out the next legitimate retry of the same content once the channel
    // drains, turning "drop one event" into "permanently miss this content".
    fn on_clipboard_change(&mut self) {
        // Hold the mutex only across the dedup-check + dispatch + state
        // advance, then release it. Keeping the guard in a tight scope
        // satisfies clippy's `significant_drop_tightening` and avoids
        // serializing unrelated readers on heavy clipboard activity.
        let mut last_copy_guard = lock_or_recover(&self.last_copy);
        if let Some(new_state) = self.dispatch_payload(&last_copy_guard) {
            *last_copy_guard = new_state;
        }
    }
}

impl ClipboardMonitor {
    /// Detect the current clipboard payload and forward it through the
    /// appropriate channel.
    ///
    /// Returns `Some(new_state)` only when the event was successfully enqueued,
    /// so the caller advances `LastCopyState` in lockstep with the channel —
    /// dropped/Full events never poison the dedup gate.
    fn dispatch_payload(&self, last_copy: &LastCopyState) -> Option<LastCopyState> {
        match detect_clipboard_payload(&self.ctx)? {
            ClipboardPayload::Files(files) => {
                let hash = hash_file_paths(&files);
                if !should_forward_files(last_copy, hash) {
                    return None;
                }
                // try_send: never block the OS clipboard callback thread.
                // If the channel is full, drop the event and warn — losing
                // a single event is preferable to stalling clipboard
                // notifications system-wide. Dedup state is left untouched
                // so the next copy of the same files can still get through.
                match self.tx.try_send(ClipboardEvent::Files(files)) {
                    Ok(()) => Some(LastCopyState::Files(hash)),
                    Err(e) => {
                        tracing::warn!(error = %e, "dropping files clipboard event (channel full or closed)");
                        None
                    }
                }
            }
            ClipboardPayload::Image(dyn_img) => {
                let hash: u64 = seahash::hash(dyn_img.as_bytes());
                if !should_forward_image(last_copy, hash) {
                    return None;
                }
                // Newest-wins overwrite: on a full single-slot channel,
                // drop the oldest queued image to make room for this one.
                match try_send_image_overwrite(&self.image_tx, &self.image_drain, (dyn_img, hash)) {
                    Ok(()) => Some(LastCopyState::Image(hash)),
                    Err(e) => {
                        tracing::warn!(error = %e, "dropping image clipboard event (processing channel closed)");
                        None
                    }
                }
            }
            ClipboardPayload::RichText {
                plain_text,
                html,
                rtf,
            } => {
                if !should_forward_text(last_copy, &plain_text) {
                    return None;
                }
                match self.tx.try_send(ClipboardEvent::RichText {
                    plain_text: plain_text.clone(),
                    html,
                    rtf,
                }) {
                    Ok(()) => Some(LastCopyState::Text(plain_text)),
                    Err(e) => {
                        tracing::warn!(error = %e, "dropping rich text clipboard event (channel full or closed)");
                        None
                    }
                }
            }
            ClipboardPayload::Text(text) => {
                if !should_forward_text(last_copy, &text) {
                    return None;
                }
                match self.tx.try_send(ClipboardEvent::Text(text.clone())) {
                    Ok(()) => Some(LastCopyState::Text(text)),
                    Err(e) => {
                        tracing::warn!(error = %e, "dropping text clipboard event (channel full or closed)");
                        None
                    }
                }
            }
        }
    }
}

/// Spawn a clipboard listener thread that watches for clipboard changes.
pub fn start_clipboard_monitor(
    tx: Sender<ClipboardEvent>,
    cx: &App,
    last_copy: Arc<Mutex<LastCopyState>>,
) {
    let (image_tx, image_rx) =
        async_channel::bounded::<(DynamicImage, u64)>(IMAGE_PROCESSING_CHANNEL_CAPACITY);
    // Producer keeps a clone of the receiver as a drain handle so the OS
    // callback thread can evict the oldest queued image when the single-slot
    // channel is full. The encoder task also holds `image_rx` and is the
    // primary consumer of meaningful events.
    let image_drain = image_rx.clone();
    let Some(monitor) = ClipboardMonitor::new(tx.clone(), image_tx, image_drain, last_copy) else {
        return;
    };

    cx.background_spawn(async move {
        while let Ok((image, hash)) = image_rx.recv().await {
            if let Some(path) = super::save_image(&image, hash) {
                // This task is async, so awaiting send() here is safe and
                // applies natural backpressure to image processing without
                // blocking the OS clipboard callback thread.
                if let Err(e) = tx.send(ClipboardEvent::Image(path, hash)).await {
                    tracing::warn!(error = %e, "failed to send image event to clipboard channel");
                }
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
#[allow(clippy::expect_used, clippy::unwrap_used)]
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

    /// Build a tiny `DynamicImage` for tests. Avoids decoding real image data.
    fn make_test_image(seed: u8) -> DynamicImage {
        let buf = image::ImageBuffer::from_pixel(1, 1, image::Rgba([seed, seed, seed, 255]));
        DynamicImage::ImageRgba8(buf)
    }

    #[test]
    fn test_try_send_image_overwrite_when_slot_empty_enqueues_payload() {
        let (tx, rx) = async_channel::bounded::<(DynamicImage, u64)>(1);

        let result = try_send_image_overwrite(&tx, &rx, (make_test_image(1), 100));

        assert!(result.is_ok());
        assert_eq!(rx.len(), 1);
        let (_img, hash) = rx.try_recv().expect("payload should be enqueued");
        assert_eq!(hash, 100);
    }

    #[test]
    fn test_try_send_image_overwrite_when_slot_full_drops_oldest_keeps_newest() {
        let (tx, rx) = async_channel::bounded::<(DynamicImage, u64)>(1);

        // Fill the slot with an "old" image.
        try_send_image_overwrite(&tx, &rx, (make_test_image(1), 1))
            .expect("Failed to enqueue first image");

        // Second push must succeed by overwriting (newest-wins).
        let result = try_send_image_overwrite(&tx, &rx, (make_test_image(2), 2));

        assert!(result.is_ok());
        assert_eq!(rx.len(), 1);
        let (_img, hash) = rx.try_recv().expect("newest payload should be enqueued");
        assert_eq!(hash, 2, "channel should retain the newest image");
    }

    #[test]
    fn test_try_send_image_overwrite_when_channel_closed_returns_error() {
        let (tx, rx) = async_channel::bounded::<(DynamicImage, u64)>(1);
        tx.close();

        let result = try_send_image_overwrite(&tx, &rx, (make_test_image(1), 1));

        assert!(matches!(
            result,
            Err(async_channel::TrySendError::Closed(_))
        ));
    }

    /// Mirrors the `Text` branch of `on_clipboard_change`: when `try_send`
    /// fails, `LastCopyState` must NOT advance, so a subsequent identical
    /// copy can still be forwarded once the channel drains.
    #[test]
    fn test_text_branch_when_send_fails_does_not_advance_last_copy_state() {
        let (tx, rx) = async_channel::bounded::<ClipboardEvent>(1);
        // Saturate the channel so the next try_send returns Full.
        tx.try_send(ClipboardEvent::Text("filler".to_string()))
            .expect("Failed to fill channel");

        let mut last_copy = LastCopyState::Text(String::new());
        let new_text = "hello".to_string();

        // Replicate the production logic: only advance state on Ok.
        if should_forward_text(&last_copy, &new_text)
            && tx.try_send(ClipboardEvent::Text(new_text.clone())).is_ok()
        {
            last_copy = LastCopyState::Text(new_text);
        }

        assert!(matches!(last_copy, LastCopyState::Text(ref t) if t.is_empty()));
        // After draining, the same content must still be forwardable.
        let _ = rx.try_recv();
        assert!(should_forward_text(&last_copy, "hello"));
    }

    /// Mirrors the `Files` branch: dropped event must not poison the dedup
    /// gate against the same files arriving again.
    #[test]
    fn test_files_branch_when_send_fails_does_not_advance_last_copy_state() {
        let (tx, _rx) = async_channel::bounded::<ClipboardEvent>(1);
        tx.try_send(ClipboardEvent::Text("filler".to_string()))
            .expect("Failed to fill channel");

        let mut last_copy = LastCopyState::Files(0);
        let files = vec!["/tmp/a".to_string()];
        let hash = hash_file_paths(&files);

        if should_forward_files(&last_copy, hash)
            && tx.try_send(ClipboardEvent::Files(files)).is_ok()
        {
            last_copy = LastCopyState::Files(hash);
        }

        // State stayed at the original hash (0), so the next retry of the
        // same files (hash) still passes the dedup gate.
        assert!(matches!(last_copy, LastCopyState::Files(0)));
        assert!(should_forward_files(&last_copy, hash));
    }

    #[test]
    fn test_text_branch_when_send_succeeds_advances_last_copy_state() {
        let (tx, _rx) = async_channel::bounded::<ClipboardEvent>(8);

        let mut last_copy = LastCopyState::Text(String::new());
        let new_text = "hello".to_string();

        if should_forward_text(&last_copy, &new_text)
            && tx.try_send(ClipboardEvent::Text(new_text.clone())).is_ok()
        {
            last_copy = LastCopyState::Text(new_text);
        }

        assert!(matches!(last_copy, LastCopyState::Text(ref t) if t == "hello"));
    }
}

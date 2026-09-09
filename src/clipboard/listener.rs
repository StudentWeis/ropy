//! A simple clipboard change listener using event-driven watching.

use std::{
    hash::Hasher,
    sync::{Arc, Mutex},
};

use async_channel::{Receiver, Sender};
use clipboard_rs::{
    Clipboard, ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
    ContentFormat, common::RustImage,
};
use gpui::{App, AppContext as _};
use image::DynamicImage;

use super::{
    ClipboardCapture, ClipboardEvent, CopyTracker, LastCopyState, capture::CopyAttempt,
    utils::ImageSaveError,
};
use crate::{
    repository::ContentType,
    utils::{content_hash, hash_file_paths, normalize_file_paths, serialize_file_paths},
};

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
fn try_send_image_overwrite<T>(
    image_tx: &Sender<T>,
    image_drain: &Receiver<T>,
    payload: T,
) -> Result<(), async_channel::TrySendError<T>> {
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
    tx: Sender<ClipboardCapture>,
    image_tx: Sender<ImageCapture>,
    /// Producer-side handle on the image channel used solely to drop the
    /// oldest queued image when the single-slot channel is full
    /// (newest-wins overwrite policy). Cloned from the same channel as
    /// `image_tx`, so this neither prevents the consumer task from receiving
    /// nor counts as an additional consumer of meaningful events.
    image_drain: Receiver<ImageCapture>,
    ctx: ClipboardContext,
    last_copy: Arc<Mutex<CopyTracker>>,
}

impl ClipboardMonitor {
    fn new(
        tx: Sender<ClipboardCapture>,
        image_tx: Sender<ImageCapture>,
        image_drain: Receiver<ImageCapture>,
        last_copy: Arc<Mutex<CopyTracker>>,
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

fn write_optional_content(hasher: &mut seahash::SeaHasher, content: Option<&str>) {
    match content {
        Some(value) => {
            hasher.write_u8(1);
            hasher.write_usize(value.len());
            hasher.write(value.as_bytes());
        }
        None => hasher.write_u8(0),
    }
}

fn rich_text_content_hash(plain_text: &str, html: Option<&str>, rtf: Option<&str>) -> u64 {
    let mut hasher = seahash::SeaHasher::new();
    hasher.write_usize(plain_text.len());
    hasher.write(plain_text.as_bytes());
    write_optional_content(&mut hasher, html);
    write_optional_content(&mut hasher, rtf);
    hasher.finish()
}

fn image_content_hash(image: &DynamicImage) -> u64 {
    let mut hasher = seahash::SeaHasher::new();
    hasher.write_u32(image.width());
    hasher.write_u32(image.height());
    let color = image.color();
    hasher.write_u8(color.bytes_per_pixel());
    hasher.write_u8(u8::from(color.has_alpha()));
    hasher.write_u8(u8::from(color.has_color()));
    hasher.write(image.as_bytes());
    hasher.finish()
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
    fn on_clipboard_change(&mut self) {
        if let Some(payload) = detect_clipboard_payload(&self.ctx) {
            dispatch_payload(
                payload,
                &self.tx,
                &self.image_tx,
                &self.image_drain,
                &self.last_copy,
            );
        }
    }
}

#[derive(Debug)]
struct ImageCapture {
    image: DynamicImage,
    hash: u64,
    attempt: CopyAttempt,
}

impl ImageCapture {
    fn encode(
        self,
        save: impl FnOnce(&DynamicImage, u64) -> Result<String, ImageSaveError>,
    ) -> Result<ClipboardCapture, ImageSaveError> {
        let path = save(&self.image, self.hash)?;
        Ok(ClipboardCapture {
            event: ClipboardEvent::Image(path, self.hash),
            attempt: self.attempt,
        })
    }
}

fn dispatch_payload(
    payload: ClipboardPayload,
    tx: &Sender<ClipboardCapture>,
    image_tx: &Sender<ImageCapture>,
    image_drain: &Receiver<ImageCapture>,
    last_copy: &Arc<Mutex<CopyTracker>>,
) {
    let (event, identity, record_id) = match payload {
        ClipboardPayload::Image(image) => {
            let hash = image_content_hash(&image);
            if let Some(attempt) = CopyTracker::begin(last_copy, LastCopyState::Image(hash), hash)
                && let Err(error) = try_send_image_overwrite(
                    image_tx,
                    image_drain,
                    ImageCapture {
                        image,
                        hash,
                        attempt,
                    },
                )
            {
                tracing::warn!(%error, "dropping image capture (processing channel unavailable)");
            }
            return;
        }
        ClipboardPayload::Text(text) => {
            let id = content_hash(&text, &ContentType::Text);
            let identity = LastCopyState::Text(text.clone());
            (ClipboardEvent::Text(text), identity, id)
        }
        ClipboardPayload::Files(files) => {
            let files = normalize_file_paths(&files);
            if files.is_empty() {
                return;
            }
            let Ok(content) = serialize_file_paths(&files) else {
                return;
            };
            let id = content_hash(&content, &ContentType::FilePath);
            let identity = LastCopyState::Files(hash_file_paths(&files));
            (ClipboardEvent::Files(files), identity, id)
        }
        ClipboardPayload::RichText {
            plain_text,
            html,
            rtf,
        } => {
            let id = content_hash(&plain_text, &ContentType::RichText);
            let identity = LastCopyState::RichText(rich_text_content_hash(
                &plain_text,
                html.as_deref(),
                rtf.as_deref(),
            ));
            (
                ClipboardEvent::RichText {
                    plain_text,
                    html,
                    rtf,
                },
                identity,
                id,
            )
        }
    };
    if let Some(attempt) = CopyTracker::begin(last_copy, identity, record_id)
        && let Err(error) = tx.try_send(ClipboardCapture { event, attempt })
    {
        tracing::warn!(%error, "dropping clipboard capture (channel full or closed)");
    }
}

/// Spawn a clipboard listener thread that watches for clipboard changes.
pub(crate) fn start_clipboard_monitor(
    tx: Sender<ClipboardCapture>,
    cx: &App,
    last_copy: Arc<Mutex<CopyTracker>>,
) {
    let (image_tx, image_rx) =
        async_channel::bounded::<ImageCapture>(IMAGE_PROCESSING_CHANNEL_CAPACITY);
    // Producer keeps a clone of the receiver as a drain handle so the OS
    // callback thread can evict the oldest queued image when the single-slot
    // channel is full. The encoder task also holds `image_rx` and is the
    // primary consumer of meaningful events.
    let image_drain = image_rx.clone();
    let Some(monitor) = ClipboardMonitor::new(tx.clone(), image_tx, image_drain, last_copy) else {
        return;
    };

    cx.background_spawn(async move {
        while let Ok(image) = image_rx.recv().await {
            match image.encode(super::save_image) {
                Ok(capture) => {
                    if let Err(error) = tx.send(capture).await {
                        tracing::warn!(%error, "failed to send image capture");
                    }
                }
                Err(error) => tracing::warn!(%error, "failed to encode clipboard image"),
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
#[expect(clippy::expect_used)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn test_dispatch_payload_failed_capture_allows_identical_retry() {
        let (tx, rx) = async_channel::bounded(1);
        let (image_tx, image_rx) = async_channel::bounded(1);
        let last_copy = Arc::new(Mutex::new(CopyTracker::default()));
        dispatch_payload(
            ClipboardPayload::Text("retry".into()),
            &tx,
            &image_tx,
            &image_rx,
            &last_copy,
        );
        // The persistence consumer failed and discarded this event.
        drop(rx.try_recv().expect("first capture"));
        dispatch_payload(
            ClipboardPayload::Text("retry".into()),
            &tx,
            &image_tx,
            &image_rx,
            &last_copy,
        );
        assert!(rx.try_recv().is_ok());
    }

    struct CaptureFixture {
        dir: tempfile::TempDir,
        backend: crate::repository::backend::memory::MemoryBackend,
        repo: crate::repository::ClipboardRepository<
            crate::repository::backend::memory::MemoryBackend,
        >,
        tracker: Arc<Mutex<CopyTracker>>,
        tx: Sender<ClipboardCapture>,
        rx: Receiver<ClipboardCapture>,
        image_tx: Sender<ImageCapture>,
        image_rx: Receiver<ImageCapture>,
    }

    impl CaptureFixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().expect("fixture directory");
            let backend = crate::repository::backend::memory::MemoryBackend::new();
            let repo = crate::repository::ClipboardRepository::from_backend(
                backend.clone(),
                dir.path().join("images"),
            )
            .expect("fixture repository");
            let (tx, rx) = async_channel::bounded(1);
            let (image_tx, image_rx) = async_channel::bounded(1);
            Self {
                dir,
                backend,
                repo,
                tracker: Arc::default(),
                tx,
                rx,
                image_tx,
                image_rx,
            }
        }

        fn dispatch(&self, payload: ClipboardPayload) {
            dispatch_payload(
                payload,
                &self.tx,
                &self.image_tx,
                &self.image_rx,
                &self.tracker,
            );
        }

        fn text(&self, text: &str) {
            self.dispatch(ClipboardPayload::Text(text.into()));
        }
    }

    fn fixture_payload(kind: ClipboardPayloadKind) -> ClipboardPayload {
        match kind {
            ClipboardPayloadKind::Text => ClipboardPayload::Text("retry".into()),
            ClipboardPayloadKind::Files => ClipboardPayload::Files(vec!["/fixture/a".into()]),
            ClipboardPayloadKind::RichText => ClipboardPayload::RichText {
                plain_text: "retry".into(),
                html: Some("<b>retry</b>".into()),
                rtf: None,
            },
            ClipboardPayloadKind::Image => ClipboardPayload::Image(make_test_image(1)),
        }
    }

    #[rstest]
    #[case(ClipboardPayloadKind::Text)]
    #[case(ClipboardPayloadKind::Files)]
    #[case(ClipboardPayloadKind::RichText)]
    fn test_dispatch_committed_then_deleted_same_content_is_recaptured(
        #[case] kind: ClipboardPayloadKind,
    ) {
        let fixture = CaptureFixture::new();
        fixture.dispatch(fixture_payload(kind));
        let record = fixture
            .rx
            .try_recv()
            .expect("capture")
            .persist(&fixture.repo)
            .expect("persist");
        fixture.dispatch(fixture_payload(kind));
        assert!(fixture.rx.is_empty(), "committed duplicates are suppressed");
        super::super::delete_tracked_record(&fixture.repo, record.id, &fixture.tracker)
            .expect("delete");
        fixture.dispatch(fixture_payload(kind));
        fixture
            .rx
            .try_recv()
            .expect("recaptured")
            .persist(&fixture.repo)
            .expect("persist retry");
        assert_eq!(fixture.repo.count(), 1);
    }

    #[test]
    fn test_dispatch_database_failure_same_content_is_retryable() {
        let fixture = CaptureFixture::new();
        fixture.text("retry");
        fixture.backend.fail_next_batch();
        assert!(
            fixture
                .rx
                .try_recv()
                .expect("capture")
                .persist(&fixture.repo)
                .is_err()
        );
        fixture.text("retry");
        fixture
            .rx
            .try_recv()
            .expect("retry")
            .persist(&fixture.repo)
            .expect("persist retry");
        assert_eq!(fixture.repo.count(), 1);
    }

    #[test]
    fn test_dispatch_failed_or_unrelated_deletion_retains_dedup() {
        let fixture = CaptureFixture::new();
        let other = fixture
            .repo
            .save_text("other".into())
            .expect("other record");
        fixture.text("retry");
        let record = fixture
            .rx
            .try_recv()
            .expect("capture")
            .persist(&fixture.repo)
            .expect("persist");
        super::super::delete_tracked_record(&fixture.repo, other.id, &fixture.tracker)
            .expect("unrelated deletion");
        fixture.backend.fail_next_batch();
        assert!(
            super::super::delete_tracked_record(&fixture.repo, record.id, &fixture.tracker)
                .is_err()
        );
        fixture.text("retry");
        assert!(fixture.rx.is_empty());
    }

    #[test]
    fn test_dispatch_late_failure_does_not_invalidate_new_generation_of_same_content() {
        let fixture = CaptureFixture::new();
        fixture.text("a");
        let old_a = fixture.rx.try_recv().expect("old a");
        fixture.text("b");
        let b = fixture.rx.try_recv().expect("b");
        fixture.text("a");
        fixture
            .rx
            .try_recv()
            .expect("new a")
            .persist(&fixture.repo)
            .expect("commit new a");
        drop(old_a);
        drop(b);
        fixture.text("a");
        assert!(fixture.rx.is_empty());
    }

    #[rstest]
    #[case(ClipboardPayloadKind::Text)]
    #[case(ClipboardPayloadKind::Files)]
    #[case(ClipboardPayloadKind::RichText)]
    fn test_dispatch_full_channel_allows_retry_after_drain(#[case] kind: ClipboardPayloadKind) {
        let fixture = CaptureFixture::new();
        fixture.text("filler");
        fixture.dispatch(fixture_payload(kind));
        drop(fixture.rx.try_recv().expect("drain filler"));
        fixture.dispatch(fixture_payload(kind));
        assert!(fixture.rx.try_recv().is_ok());
    }

    #[test]
    fn test_dispatch_image_encoding_failure_then_delete_allows_retry() {
        let fixture = CaptureFixture::new();
        fixture.dispatch(fixture_payload(ClipboardPayloadKind::Image));
        let capture = fixture.image_rx.try_recv().expect("image capture");
        assert!(
            capture
                .encode(|_, _| Err(std::io::Error::other("injected encoder failure").into()))
                .is_err()
        );
        fixture.dispatch(fixture_payload(ClipboardPayloadKind::Image));
        let capture = fixture.image_rx.try_recv().expect("image retry");
        let path = fixture.dir.path().join("image.png");
        let record = capture
            .encode(|image, _| {
                image.save_with_format(&path, image::ImageFormat::Png)?;
                Ok(path.to_string_lossy().into_owned())
            })
            .expect("encode")
            .persist(&fixture.repo)
            .expect("persist image");
        super::super::delete_tracked_record(&fixture.repo, record.id, &fixture.tracker)
            .expect("delete image");
        fixture.dispatch(fixture_payload(ClipboardPayloadKind::Image));
        assert!(fixture.image_rx.try_recv().is_ok());
    }

    #[test]
    fn test_dispatch_image_overwrite_and_late_failure_preserves_newest_attempt() {
        let fixture = CaptureFixture::new();
        fixture.dispatch(ClipboardPayload::Image(make_test_image(1)));
        let encoding = fixture.image_rx.try_recv().expect("encoding image");
        fixture.dispatch(ClipboardPayload::Image(make_test_image(2)));
        fixture.dispatch(ClipboardPayload::Image(make_test_image(3)));
        assert!(
            encoding
                .encode(|_, _| Err(std::io::Error::other("late failure").into()))
                .is_err()
        );
        let newest = fixture.image_rx.try_recv().expect("newest image");
        assert_eq!(newest.hash, image_content_hash(&make_test_image(3)));
        fixture.dispatch(ClipboardPayload::Image(make_test_image(3)));
        assert!(
            fixture.image_rx.is_empty(),
            "pending identical copy is suppressed"
        );
        drop(newest);
        fixture.dispatch(ClipboardPayload::Image(make_test_image(3)));
        assert!(fixture.image_rx.try_recv().is_ok());
    }

    #[test]
    fn test_dispatch_rich_text_changed_markup_is_recaptured() {
        let fixture = CaptureFixture::new();
        fixture.dispatch(fixture_payload(ClipboardPayloadKind::RichText));
        fixture
            .rx
            .try_recv()
            .expect("capture")
            .persist(&fixture.repo)
            .expect("persist");
        fixture.dispatch(ClipboardPayload::RichText {
            plain_text: "retry".into(),
            html: Some("<i>retry</i>".into()),
            rtf: None,
        });
        assert!(fixture.rx.try_recv().is_ok());
    }

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
    fn test_image_content_hash_when_dimensions_differ_returns_different_hashes() {
        let horizontal = DynamicImage::ImageRgba8(
            image::ImageBuffer::from_raw(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8])
                .expect("valid horizontal image"),
        );
        let vertical = DynamicImage::ImageRgba8(
            image::ImageBuffer::from_raw(1, 2, vec![1, 2, 3, 4, 5, 6, 7, 8])
                .expect("valid vertical image"),
        );

        assert_ne!(
            image_content_hash(&horizontal),
            image_content_hash(&vertical)
        );
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
}

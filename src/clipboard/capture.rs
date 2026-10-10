//! Track capture attempts until persistence acknowledges them.
//!
//! Dropping a capture at any pipeline stage makes only that attempt retryable,
//! including queue eviction.

use std::sync::{Arc, Mutex};

use super::{ClipboardEvent, LastCopyState};
use crate::{
    repository::{
        ClipboardRecord, ClipboardRepository, backend::StorageBackend, errors::RepositoryError,
    },
    utils::lock_or_recover,
};

#[derive(Debug, Default)]
pub(crate) struct CopyTracker {
    last: Option<TrackedCopy>,
}

#[derive(Debug)]
struct TrackedCopy {
    content: LastCopyState,
    record_id: u64,
    generation: Arc<()>,
}

impl CopyTracker {
    pub(super) fn begin(
        shared: &Arc<Mutex<Self>>,
        content: LastCopyState,
        record_id: u64,
    ) -> Option<CopyAttempt> {
        let generation = Arc::new(());
        {
            let mut tracker = lock_or_recover(shared);
            if tracker
                .last
                .as_ref()
                .is_some_and(|last| last.content == content)
            {
                return None;
            }
            tracker.last = Some(TrackedCopy {
                content,
                record_id,
                generation: generation.clone(),
            });
        }
        Some(CopyAttempt {
            tracker: shared.clone(),
            generation,
            committed: false,
        })
    }

    pub(crate) fn clear(&mut self) {
        self.last = None;
    }

    fn invalidate_record(&mut self, id: u64) {
        if self.last.as_ref().is_some_and(|last| last.record_id == id) {
            self.clear();
        }
    }
}

#[derive(Debug)]
pub(super) struct CopyAttempt {
    tracker: Arc<Mutex<CopyTracker>>,
    generation: Arc<()>,
    committed: bool,
}

impl Drop for CopyAttempt {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        let mut tracker = lock_or_recover(&self.tracker);
        if tracker
            .last
            .as_ref()
            .is_some_and(|last| Arc::ptr_eq(&last.generation, &self.generation))
        {
            tracker.clear();
        }
    }
}

#[derive(Debug)]
pub(crate) struct ClipboardCapture {
    pub(super) event: ClipboardEvent,
    pub(super) attempt: CopyAttempt,
}

impl ClipboardCapture {
    pub(crate) fn persist<B: StorageBackend>(
        self,
        repo: &ClipboardRepository<B>,
    ) -> Result<ClipboardRecord, RepositoryError> {
        let Self { event, mut attempt } = self;
        let record = match event {
            ClipboardEvent::Text(text) => repo.save_text(text),
            ClipboardEvent::Image(image) => repo.save_pending_image(image),
            ClipboardEvent::Files(paths) => repo.save_files(&paths),
            ClipboardEvent::RichText {
                plain_text,
                html,
                rtf,
            } => repo.save_rich_text(plain_text, html.as_deref(), rtf.as_deref()),
        }?;
        attempt.committed = true;
        Ok(record)
    }
}

pub(crate) fn delete_tracked_record<B: StorageBackend>(
    repo: &ClipboardRepository<B>,
    id: u64,
    tracker: &Mutex<CopyTracker>,
) -> Result<bool, RepositoryError> {
    let removed = repo.delete(id)?;
    if removed {
        lock_or_recover(tracker).invalidate_record(id);
    }
    Ok(removed)
}

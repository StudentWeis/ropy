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
    pending: Vec<(u64, Arc<()>)>,
    operation: Arc<Mutex<()>>,
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
            tracker.pending.push((record_id, generation.clone()));
        }
        Some(CopyAttempt {
            tracker: shared.clone(),
            generation,
            committed: false,
            captured_at: chrono::Local::now(),
        })
    }

    fn clear(&mut self) {
        self.last = None;
        self.pending.clear();
    }

    fn invalidate_record(&mut self, id: u64) {
        if self.last.as_ref().is_some_and(|last| last.record_id == id) {
            self.last = None;
        }
        self.pending.retain(|(record_id, _)| *record_id != id);
    }
}

#[derive(Debug)]
pub(super) struct CopyAttempt {
    tracker: Arc<Mutex<CopyTracker>>,
    generation: Arc<()>,
    committed: bool,
    captured_at: chrono::DateTime<chrono::Local>,
}

impl Drop for CopyAttempt {
    fn drop(&mut self) {
        let mut tracker = lock_or_recover(&self.tracker);
        tracker
            .pending
            .retain(|(_, generation)| !Arc::ptr_eq(generation, &self.generation));
        if !self.committed
            && tracker
                .last
                .as_ref()
                .is_some_and(|last| Arc::ptr_eq(&last.generation, &self.generation))
        {
            tracker.last = None;
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
    ) -> Result<Option<ClipboardRecord>, RepositoryError> {
        let Self { event, mut attempt } = self;
        // Serialize persistence with history clear/delete without blocking
        // new clipboard callbacks on storage I/O. Never wait for the operation
        // lock while holding the tracker lock.
        let operation = lock_or_recover(&attempt.tracker).operation.clone();
        let _operation = lock_or_recover(&operation);
        let valid = lock_or_recover(&attempt.tracker)
            .pending
            .iter()
            .any(|(_, generation)| Arc::ptr_eq(generation, &attempt.generation));
        if !valid {
            return Ok(None);
        }
        let captured_at = attempt.captured_at;
        let record = match event {
            ClipboardEvent::Text(text) => {
                repo.save_at(text, crate::repository::ContentType::Text, captured_at)
            }
            ClipboardEvent::Image(image) => repo.save_pending_image_at(image, captured_at),
            ClipboardEvent::Files(paths) => repo.save_files_at(&paths, captured_at),
            ClipboardEvent::RichText {
                plain_text,
                html,
                rtf,
            } => repo.save_rich_text_at(plain_text, html.as_deref(), rtf.as_deref(), captured_at),
        }?;
        attempt.committed = true;
        Ok(Some(record))
    }
}

pub(crate) fn clear_tracked_history<B: StorageBackend>(
    repo: &ClipboardRepository<B>,
    tracker: &Mutex<CopyTracker>,
) -> Result<(), RepositoryError> {
    let operation = lock_or_recover(tracker).operation.clone();
    let _operation = lock_or_recover(&operation);
    repo.clear()?;
    lock_or_recover(tracker).clear();
    Ok(())
}

pub(crate) fn clear_tracked_ordinary_history<B: StorageBackend>(
    repo: &ClipboardRepository<B>,
    tracker: &Mutex<CopyTracker>,
) -> Result<(), RepositoryError> {
    let operation = lock_or_recover(tracker).operation.clone();
    let _operation = lock_or_recover(&operation);
    let result = repo.clear_ordinary_records();
    // Ordinary cleanup can delete some records before returning an error.
    // Invalidate pending captures even then so those deletions stay effective.
    lock_or_recover(tracker).clear();
    result.map(|_| ())
}

pub(crate) fn delete_tracked_record<B: StorageBackend>(
    repo: &ClipboardRepository<B>,
    id: u64,
    tracker: &Mutex<CopyTracker>,
) -> Result<bool, RepositoryError> {
    let operation = lock_or_recover(tracker).operation.clone();
    let _operation = lock_or_recover(&operation);
    let removed = repo.delete(id)?;
    // A retention cleanup may already have removed the selected record.
    // A successful delete still cancels pending copies of that record.
    lock_or_recover(tracker).invalidate_record(id);
    Ok(removed)
}

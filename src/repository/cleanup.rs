//! Cleanup strategies for the clipboard repository.

use std::collections::HashSet;

use super::{
    errors::RepositoryError,
    models::{ClipboardRecord, ContentType},
    repo::ClipboardRepository,
};

/// Allow the repository to grow slightly past the configured limit so cleanup
/// can batch deletions instead of scanning on every successful save.
const CLEANUP_BUFFER_DIVISOR: usize = 10;
const MIN_CLEANUP_BUFFER_RECORDS: usize = 1;

impl ClipboardRepository {
    /// Clear all ordinary records while preserving pinned and favorited ones.
    pub fn clear_ordinary_records(&self) -> Result<usize, RepositoryError> {
        let total = self.count();
        let favorite_ids = self.favorite_id_set()?;
        let ordinary_total = self.ordinary_record_count(total, &favorite_ids)?;

        self.cleanup_old_records_with_ordinary_total(0, ordinary_total, total, &favorite_ids)
    }

    /// Clean up old records, keeping the most recent `keep_count` records.
    ///
    /// Pinned records are never removed.
    pub fn cleanup_old_records(&self, keep_count: usize) -> Result<usize, RepositoryError> {
        let total = self.count();
        let favorite_ids = self.favorite_id_set()?;
        let ordinary_total = self.ordinary_record_count(total, &favorite_ids)?;
        self.cleanup_old_records_with_ordinary_total(
            keep_count,
            ordinary_total,
            total,
            &favorite_ids,
        )
    }

    /// Clean up old records only after the repository has exceeded the
    /// configured limit by a small buffer.
    pub fn cleanup_old_records_if_needed(
        &self,
        keep_count: usize,
    ) -> Result<usize, RepositoryError> {
        let total = self.count();
        let favorite_ids = self.favorite_id_set()?;
        let ordinary_total = self.ordinary_record_count(total, &favorite_ids)?;
        if ordinary_total <= Self::cleanup_trigger_record_count(keep_count) {
            return Ok(0);
        }

        self.cleanup_old_records_with_ordinary_total(
            keep_count,
            ordinary_total,
            total,
            &favorite_ids,
        )
    }

    fn cleanup_old_records_with_ordinary_total(
        &self,
        keep_count: usize,
        ordinary_total: usize,
        total: usize,
        favorite_ids: &HashSet<u64>,
    ) -> Result<usize, RepositoryError> {
        if ordinary_total <= keep_count {
            return Ok(0);
        }

        let candidates = self.time_index.oldest_unpinned(total)?;
        let mut removed = 0;

        for (ti_key, id) in candidates {
            if ordinary_total.saturating_sub(removed) <= keep_count {
                break;
            }
            if favorite_ids.contains(&id) {
                continue;
            }

            let rec_key = id.to_be_bytes();
            if let Some(value) = self.get_raw(&rec_key)?
                && let Ok(record) = postcard::from_bytes::<ClipboardRecord>(&value)
                && record.content_type == ContentType::Image
            {
                Self::remove_image_files(&record.content);
            }
            self.records.remove(&rec_key)?;
            self.time_index.remove_raw(&ti_key)?;
            removed += 1;
        }
        Ok(removed)
    }

    pub(super) fn cleanup_buffer_record_count(keep_count: usize) -> usize {
        keep_count
            .saturating_div(CLEANUP_BUFFER_DIVISOR)
            .max(MIN_CLEANUP_BUFFER_RECORDS)
    }

    fn cleanup_trigger_record_count(keep_count: usize) -> usize {
        keep_count.saturating_add(Self::cleanup_buffer_record_count(keep_count))
    }

    fn ordinary_record_count(
        &self,
        total: usize,
        favorite_ids: &HashSet<u64>,
    ) -> Result<usize, RepositoryError> {
        let mut special_ids = self
            .time_index
            .pinned_ids()?
            .into_iter()
            .collect::<HashSet<_>>();
        special_ids.extend(favorite_ids.iter().copied());
        Ok(total.saturating_sub(special_ids.len()))
    }
}

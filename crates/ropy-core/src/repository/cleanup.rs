#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Cleanup strategies for the clipboard repository.

use std::collections::HashSet;

use super::{
    backend::{
        RECORDS_TREE, StorageBackend, TIME_INDEX_LOOKUP_TREE, TIME_INDEX_TREE, TreeKey,
        redb::RedbBackend,
    },
    errors::RepositoryError,
    models::ClipboardRecord,
    repo::ClipboardRepository,
    sidecar::remove_record_sidecars,
};

/// Let the repository grow slightly past the configured limit so cleanup can
/// batch deletions instead of scanning on every successful save.
const CLEANUP_BUFFER_DIVISOR: usize = 10;
const MIN_CLEANUP_BUFFER_RECORDS: usize = 1;

impl<B: StorageBackend> ClipboardRepository<B> {
    /// "Clear history" without losing user-curated records: pinned and
    /// favorited entries survive.
    ///
    /// # Errors
    /// Returns an error if selecting or deleting ordinary records fails.
    pub fn clear_ordinary_records(&self) -> Result<usize, RepositoryError> {
        let _operation = self.lock_operation();
        let total = self.count();
        let favorite_ids = self.favorite_id_set()?;
        let ordinary_total = self.ordinary_record_count(total, &favorite_ids)?;

        self.cleanup_old_records_with_ordinary_total(0, ordinary_total, total, &favorite_ids)
    }

    /// Trim ordinary records down to the most recent `keep_count`. Pinned
    /// entries are skipped so users can't lose deliberately-kept records.
    ///
    /// # Errors
    /// Returns an error if querying retention exemptions or deleting old records fails.
    pub fn cleanup_old_records(&self, keep_count: usize) -> Result<usize, RepositoryError> {
        let _operation = self.lock_operation();
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

    /// Hot-path variant of [`Self::cleanup_old_records`]: skip the scan
    /// until ordinary record count exceeds `keep_count` by the buffer, so
    /// the common save → cleanup pair doesn't pay for an iteration on
    /// every insert.
    ///
    /// # Errors
    /// Returns an error if retention cleanup cannot query or delete records.
    pub fn cleanup_old_records_if_needed(
        &self,
        keep_count: usize,
    ) -> Result<usize, RepositoryError> {
        let _operation = self.lock_operation();
        let total = self.count();
        let trigger = ClipboardRepository::<RedbBackend>::cleanup_trigger_record_count(keep_count);
        // Ordinary records cannot outnumber all records. Avoid reading retention
        // exemptions until even the total exceeds the buffered limit.
        if total <= trigger {
            return Ok(0);
        }
        let favorite_ids = self.favorite_id_set()?;
        let ordinary_total = self.ordinary_record_count(total, &favorite_ids)?;
        if ordinary_total <= trigger {
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

        let mut candidates = self.time_index.oldest_unpinned(total)?;
        // Index keys have millisecond precision. Resolve ties before deciding
        // which ordinary records to delete, without changing the stored format.
        for bucket in candidates.chunk_by_mut(|(left, _), (right, _)| left[..8] == right[..8]) {
            if bucket.len() < 2 {
                continue;
            }
            let mut timed = Vec::with_capacity(bucket.len());
            for entry in bucket.iter().copied() {
                let captured_at = self
                    .get_raw(&entry.1.to_be_bytes())?
                    .and_then(|value| postcard::from_bytes::<ClipboardRecord>(&value).ok())
                    .map(|record| record.created_at);
                timed.push((entry, captured_at));
            }
            timed.sort_by_key(|(_, captured_at)| *captured_at);
            for (entry, (ordered, _)) in bucket.iter_mut().zip(timed) {
                *entry = ordered;
            }
        }
        let mut removed = 0;

        for (ti_key, id) in candidates {
            if ordinary_total.saturating_sub(removed) <= keep_count {
                break;
            }
            if favorite_ids.contains(&id) {
                continue;
            }

            let rec_key = id.to_be_bytes();
            let record = self
                .get_raw(&rec_key)?
                .and_then(|value| postcard::from_bytes::<ClipboardRecord>(&value).ok());
            // Check the current record as well as the index before deleting.
            if record.as_ref().is_some_and(|record| record.pinned) {
                continue;
            }
            self.backend.remove_batch(&[
                TreeKey::new(RECORDS_TREE, &rec_key),
                TreeKey::new(TIME_INDEX_TREE, &ti_key),
                TreeKey::new(TIME_INDEX_LOOKUP_TREE, &rec_key),
            ])?;
            if let Some(record) = record.as_ref() {
                remove_record_sidecars(record);
            }
            removed += 1;
        }
        Ok(removed)
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

impl ClipboardRepository<RedbBackend> {
    pub(super) fn cleanup_buffer_record_count(keep_count: usize) -> usize {
        keep_count
            .saturating_div(CLEANUP_BUFFER_DIVISOR)
            .max(MIN_CLEANUP_BUFFER_RECORDS)
    }

    fn cleanup_trigger_record_count(keep_count: usize) -> usize {
        keep_count.saturating_add(Self::cleanup_buffer_record_count(keep_count))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;
    use crate::repository::test_helpers::{create_test_repo, save_numbered_records, save_text_at};

    #[rstest]
    #[case(0, 1)]
    #[case(1, 1)]
    #[case(5, 1)]
    #[case(9, 1)]
    #[case(10, 1)]
    #[case(11, 1)]
    #[case(20, 2)]
    #[case(100, 10)]
    #[case(1000, 100)]
    fn test_cleanup_buffer_record_count_returns_expected(
        #[case] keep_count: usize,
        #[case] expected: usize,
    ) {
        assert_eq!(
            ClipboardRepository::cleanup_buffer_record_count(keep_count),
            expected
        );
    }

    #[rstest]
    #[case(0, 1)]
    #[case(1, 2)]
    #[case(10, 11)]
    #[case(20, 22)]
    #[case(100, 110)]
    fn test_cleanup_trigger_record_count_returns_keep_plus_buffer(
        #[case] keep_count: usize,
        #[case] expected: usize,
    ) {
        assert_eq!(
            ClipboardRepository::cleanup_trigger_record_count(keep_count),
            expected
        );
    }

    #[test]
    fn test_cleanup_old_records_when_all_favorited_removes_none() {
        let (_temp_dir, repo) = create_test_repo();

        let record_one = save_text_at(&repo, "One", 1);
        let record_two = save_text_at(&repo, "Two", 2);
        let record_three = save_text_at(&repo, "Three", 3);

        repo.toggle_favorite(record_one.id)
            .expect("Failed to favorite");
        repo.toggle_favorite(record_two.id)
            .expect("Failed to favorite");
        repo.toggle_favorite(record_three.id)
            .expect("Failed to favorite");

        let removed = repo.cleanup_old_records(1).expect("Failed to cleanup");

        assert_eq!(removed, 0);
        assert_eq!(repo.count(), 3);
    }

    #[test]
    fn test_cleanup_old_records_when_mixed_pinned_and_favorited_preserves_both() {
        let (_temp_dir, repo) = create_test_repo();

        let pinned = save_text_at(&repo, "Pinned", -2);
        let favorited = save_text_at(&repo, "Favorited", -1);
        save_numbered_records(&repo, 5);

        repo.toggle_pin(pinned.id).expect("Failed to pin");
        repo.toggle_favorite(favorited.id)
            .expect("Failed to favorite");

        let removed = repo.cleanup_old_records(2).expect("Failed to cleanup");

        assert_eq!(removed, 3);
        assert_eq!(repo.count(), 4);
        assert!(repo.get_by_id(pinned.id).expect("query").is_some());
        assert!(repo.get_by_id(favorited.id).expect("query").is_some());
    }

    #[test]
    fn test_cleanup_below_trigger_skips_retention_record_reads() {
        let (_temp_dir, repo) = create_test_repo();
        let record = repo.save_text("Favorite".into()).expect("save");
        repo.toggle_favorite(record.id).expect("favorite");
        repo.records.fail_next_get();

        assert_eq!(repo.cleanup_old_records_if_needed(10).expect("cleanup"), 0);
        // The injected failure remains pending: cleanup did not read the record.
        assert!(repo.get_by_id(record.id).is_err());
        assert!(repo.get_by_id(record.id).expect("query").is_some());
    }

    #[test]
    fn test_cleanup_old_records_if_needed_when_below_trigger_removes_none() {
        let (_temp_dir, repo) = create_test_repo();

        // keep_count=10, buffer=1, trigger=11. Save exactly 11 records.
        save_numbered_records(&repo, 11);

        let removed = repo
            .cleanup_old_records_if_needed(10)
            .expect("Failed to cleanup");

        assert_eq!(removed, 0);
        assert_eq!(repo.count(), 11);
    }

    #[test]
    fn test_cleanup_old_records_if_needed_when_above_trigger_trims_to_keep_count() {
        let (_temp_dir, repo) = create_test_repo();

        // keep_count=10, buffer=1, trigger=11. Save 12 records to exceed trigger.
        save_numbered_records(&repo, 12);

        let removed = repo
            .cleanup_old_records_if_needed(10)
            .expect("Failed to cleanup");

        assert_eq!(removed, 2);
        assert_eq!(repo.count(), 10);
    }

    #[test]
    fn test_cleanup_old_records_if_needed_when_favorited_records_present_excludes_from_ordinary_count()
     {
        let (_temp_dir, repo) = create_test_repo();

        // Save 12 records, favorite 2 of them.
        // ordinary_total = 12 - 2 = 10, trigger for keep=10 is 11, so no cleanup.
        let first = save_text_at(&repo, "First", -2);
        let second = save_text_at(&repo, "Second", -1);
        save_numbered_records(&repo, 10);

        repo.toggle_favorite(first.id).expect("Failed to favorite");
        repo.toggle_favorite(second.id).expect("Failed to favorite");

        let removed = repo
            .cleanup_old_records_if_needed(10)
            .expect("Failed to cleanup");

        assert_eq!(removed, 0);
        assert_eq!(repo.count(), 12);
    }

    #[test]
    fn test_clear_ordinary_records_when_empty_repo_returns_zero() {
        let (_temp_dir, repo) = create_test_repo();

        let removed = repo.clear_ordinary_records().expect("Failed to clear");

        assert_eq!(removed, 0);
        assert_eq!(repo.count(), 0);
    }

    #[test]
    fn test_clear_ordinary_records_when_all_ordinary_removes_all() {
        let (_temp_dir, repo) = create_test_repo();
        save_numbered_records(&repo, 5);

        let removed = repo.clear_ordinary_records().expect("Failed to clear");

        assert_eq!(removed, 5);
        assert_eq!(repo.count(), 0);
    }
}

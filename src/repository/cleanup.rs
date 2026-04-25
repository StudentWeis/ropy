#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Cleanup strategies for the clipboard repository.

use std::collections::HashSet;

use super::{errors::RepositoryError, models::ClipboardRecord, repo::ClipboardRepository};

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
            {
                Self::remove_record_sidecars(&record);
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

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use rstest::rstest;

    use super::*;
    use crate::repository::test_helpers::{create_test_repo, save_numbered_records};

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
        let repo = create_test_repo();

        let record_one = repo.save_text("One".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let record_two = repo.save_text("Two".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let record_three = repo.save_text("Three".to_string()).expect("Failed to save");

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
        let repo = create_test_repo();

        let pinned = repo
            .save_text("Pinned".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let favorited = repo
            .save_text("Favorited".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
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
    fn test_cleanup_old_records_if_needed_when_below_trigger_removes_none() {
        let repo = create_test_repo();

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
        let repo = create_test_repo();

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
        let repo = create_test_repo();

        // Save 12 records, favorite 2 of them.
        // ordinary_total = 12 - 2 = 10, trigger for keep=10 is 11, so no cleanup.
        let first = repo.save_text("First".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let second = repo
            .save_text("Second".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
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
        let repo = create_test_repo();

        let removed = repo.clear_ordinary_records().expect("Failed to clear");

        assert_eq!(removed, 0);
        assert_eq!(repo.count(), 0);
    }

    #[test]
    fn test_clear_ordinary_records_when_all_ordinary_removes_all() {
        let repo = create_test_repo();
        save_numbered_records(&repo, 5);

        let removed = repo.clear_ordinary_records().expect("Failed to clear");

        assert_eq!(removed, 5);
        assert_eq!(repo.count(), 0);
    }
}

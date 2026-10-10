//! Display ordering and query logic for the clipboard repository.

use std::{cmp::Ordering, collections::HashSet};

use super::{
    backend::{StorageBackend, redb::RedbBackend},
    errors::RepositoryError,
    models::ClipboardRecord,
    repo::ClipboardRepository,
};

/// Records and live favorites read together under the repository operation lock.
#[derive(Debug)]
pub struct DisplaySnapshot {
    records: Vec<ClipboardRecord>,
    favorite_ids: HashSet<u64>,
}

impl DisplaySnapshot {
    /// Consume the snapshot into its ordered records and favorite identifiers.
    #[must_use]
    pub fn into_parts(self) -> (Vec<ClipboardRecord>, HashSet<u64>) {
        (self.records, self.favorite_ids)
    }
}

impl<B: StorageBackend> ClipboardRepository<B> {
    /// Get the records for the default board view.
    ///
    /// Pinned records stay at the top. Favorited records do not consume the
    /// ordinary `limit`, but otherwise remain in the default chronological
    /// ordering with other unpinned records.
    ///
    /// # Errors
    /// Returns an error if favorites or the display index cannot be queried.
    pub fn get_display_records(
        &self,
        limit: usize,
    ) -> Result<Vec<ClipboardRecord>, RepositoryError> {
        Ok(self.get_display_snapshot(limit)?.records)
    }

    /// Read display records and live favorites as one coherent snapshot.
    ///
    /// # Errors
    /// Returns an error if favorites or the display index cannot be queried.
    pub fn get_display_snapshot(&self, limit: usize) -> Result<DisplaySnapshot, RepositoryError> {
        let _operation = self.lock_operation();
        let favorite_ids = self.favorite_id_set()?;
        let selected_ids = self.time_index.select_display_ids(limit, &favorite_ids)?;
        let mut records = self.load_records(&selected_ids);
        ClipboardRepository::<RedbBackend>::sort_for_display(&mut records);
        Ok(DisplaySnapshot {
            records,
            favorite_ids,
        })
    }
}

impl ClipboardRepository<RedbBackend> {
    /// Compare pinned priority first, then newest capture time.
    #[must_use]
    pub fn compare_for_display(left: &ClipboardRecord, right: &ClipboardRecord) -> Ordering {
        Self::display_priority(left)
            .cmp(&Self::display_priority(right))
            .then_with(|| right.created_at.cmp(&left.created_at))
    }

    /// Sort records by pinned priority and newest capture time.
    pub fn sort_for_display(records: &mut [ClipboardRecord]) {
        records.sort_unstable_by(Self::compare_for_display);
    }

    const fn display_priority(record: &ClipboardRecord) -> u8 {
        (!record.pinned) as u8
    }
}

#[cfg(test)]
#[expect(clippy::panic)]
mod tests {
    use std::cmp::Ordering;

    use chrono::{Local, TimeZone};

    use crate::repository::{ClipboardRecord, models::ContentType, repo::ClipboardRepository};

    fn test_datetime(hour: u32) -> chrono::DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 4, 18, hour, 0, 0)
            .single()
            .unwrap_or_else(|| panic!("invalid local datetime for test hour {hour}"))
    }

    fn test_record(content: &str, pinned: bool, hour: u32) -> ClipboardRecord {
        ClipboardRecord {
            id: 0,
            content: content.to_string(),
            content_type: ContentType::Text,
            pinned,
            created_at: test_datetime(hour),
            rich_text_meta: None,
        }
    }

    #[test]
    fn test_display_priority_pinned_returns_zero() {
        let record = test_record("pinned", true, 10);

        assert_eq!(ClipboardRepository::display_priority(&record), 0);
    }

    #[test]
    #[expect(clippy::expect_used)]
    fn test_display_snapshot_limit_keeps_favorites_and_pins_with_membership() {
        let repo = crate::repository::test_helpers::create_test_repo();
        let favorite = repo.save_text("favorite".into()).expect("favorite");
        repo.toggle_favorite(favorite.id).expect("mark favorite");
        let pinned = repo.save_text("pinned".into()).expect("pinned");
        repo.toggle_pin(pinned.id).expect("pin");
        repo.save_text("ordinary".into()).expect("ordinary");
        let (records, favorites) = repo.get_display_snapshot(0).expect("snapshot").into_parts();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].id, pinned.id);
        assert_eq!(records[1].id, favorite.id);
        assert_eq!(favorites, std::collections::HashSet::from([favorite.id]));
    }

    #[test]
    fn test_display_priority_unpinned_returns_one() {
        let record = test_record("unpinned", false, 10);

        assert_eq!(ClipboardRepository::display_priority(&record), 1);
    }

    #[test]
    fn test_compare_for_display_pinned_before_unpinned() {
        let pinned = test_record("pinned", true, 8);
        let unpinned = test_record("unpinned", false, 12);

        assert_eq!(
            ClipboardRepository::compare_for_display(&pinned, &unpinned),
            Ordering::Less
        );
        assert_eq!(
            ClipboardRepository::compare_for_display(&unpinned, &pinned),
            Ordering::Greater
        );
    }

    #[test]
    fn test_compare_for_display_same_priority_newer_first() {
        let older = test_record("older", false, 8);
        let newer = test_record("newer", false, 12);

        assert_eq!(
            ClipboardRepository::compare_for_display(&newer, &older),
            Ordering::Less
        );
        assert_eq!(
            ClipboardRepository::compare_for_display(&older, &newer),
            Ordering::Greater
        );
    }

    #[test]
    fn test_compare_for_display_same_priority_same_time_returns_equal() {
        let record_a = test_record("a", false, 10);
        let record_b = test_record("b", false, 10);

        assert_eq!(
            ClipboardRepository::compare_for_display(&record_a, &record_b),
            Ordering::Equal
        );
    }

    #[test]
    fn test_sort_for_display_mixed_records_pinned_first_then_newest() {
        let mut records = vec![
            test_record("old unpinned", false, 8),
            test_record("new unpinned", false, 12),
            test_record("old pinned", true, 6),
            test_record("new pinned", true, 14),
        ];

        ClipboardRepository::sort_for_display(&mut records);

        let contents: Vec<&str> = records.iter().map(|r| r.content.as_str()).collect();
        assert_eq!(
            contents,
            vec!["new pinned", "old pinned", "new unpinned", "old unpinned"]
        );
    }

    #[test]
    fn test_sort_for_display_empty_slice_does_not_panic() {
        let mut records: Vec<ClipboardRecord> = vec![];

        ClipboardRepository::sort_for_display(&mut records);

        assert_eq!(records, []);
    }

    #[test]
    fn test_sort_for_display_single_record_unchanged() {
        let mut records = vec![test_record("only", false, 10)];

        ClipboardRepository::sort_for_display(&mut records);

        assert_eq!(records[0].content, "only");
    }
}

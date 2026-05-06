//! Display ordering and query logic for the clipboard repository.

use std::cmp::Ordering;

use super::{
    backend::StorageBackend, errors::RepositoryError, models::ClipboardRecord,
    redb_backend::RedbBackend, repo::ClipboardRepository,
};

impl<B: StorageBackend> ClipboardRepository<B> {
    /// Get the records for the default board view.
    ///
    /// Pinned records stay at the top. Favorited records do not consume the
    /// ordinary `limit`, but otherwise remain in the default chronological
    /// ordering with other unpinned records.
    pub(crate) fn get_display_records(
        &self,
        limit: usize,
    ) -> Result<Vec<ClipboardRecord>, RepositoryError> {
        let favorite_ids = self.favorite_id_set()?;
        let selected_ids = self.time_index.select_display_ids(limit, &favorite_ids)?;
        let mut records = self.load_records(&selected_ids);
        ClipboardRepository::<RedbBackend>::sort_for_display(&mut records);
        Ok(records)
    }
}

impl ClipboardRepository<RedbBackend> {
    pub(crate) fn compare_for_display(left: &ClipboardRecord, right: &ClipboardRecord) -> Ordering {
        Self::display_priority(left)
            .cmp(&Self::display_priority(right))
            .then_with(|| right.created_at.cmp(&left.created_at))
    }

    pub(crate) fn sort_for_display(records: &mut [ClipboardRecord]) {
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

        assert!(records.is_empty());
    }

    #[test]
    fn test_sort_for_display_single_record_unchanged() {
        let mut records = vec![test_record("only", false, 10)];

        ClipboardRepository::sort_for_display(&mut records);

        assert_eq!(records[0].content, "only");
    }
}

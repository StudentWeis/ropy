//! Display ordering and query logic for the clipboard repository.

use std::cmp::Ordering;

use super::{errors::RepositoryError, models::ClipboardRecord, repo::ClipboardRepository};

impl ClipboardRepository {
    /// Get the records for the default board view.
    ///
    /// Pinned records stay at the top. Favorited records do not consume the
    /// ordinary `limit`, but otherwise remain in the default chronological
    /// ordering with other unpinned records.
    pub fn get_display_records(
        &self,
        limit: usize,
    ) -> Result<Vec<ClipboardRecord>, RepositoryError> {
        let favorite_ids = self.favorite_id_set()?;
        let selected_ids = self.time_index.select_display_ids(limit, &favorite_ids)?;
        let mut records = self.load_records(&selected_ids);
        Self::sort_for_display(&mut records);
        Ok(records)
    }

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

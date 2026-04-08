use std::collections::HashSet;

use super::search::{ContentFilter, SearchOptions, filter_records_by_query};
use crate::repository::{ClipboardRecord, ClipboardRepository};

/// Sort and filter record indices by search query, content type, and display order.
pub(super) fn filter_and_sort_record_indices(
    records: &[ClipboardRecord],
    query: &str,
    content_filter: ContentFilter,
    search_options: SearchOptions,
    favorite_ids: &HashSet<u64>,
    favorites_only: bool,
) -> Vec<usize> {
    let mut filtered_indices = filter_records_by_query(
        records,
        query,
        content_filter,
        search_options,
        favorite_ids,
        favorites_only,
    );

    filtered_indices.sort_unstable_by(|left_index, right_index| {
        let left = records.get(*left_index);
        let right = records.get(*right_index);

        match (left, right) {
            (Some(left), Some(right)) => ClipboardRepository::compare_for_display(left, right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left_index.cmp(right_index),
        }
    });

    filtered_indices
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::gui) enum ClearConfirmAction {
    AllHistory,
    OrdinaryRecords,
}

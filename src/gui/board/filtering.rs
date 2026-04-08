use std::collections::HashSet;

use super::search::{ContentFilter, SearchOptions, filter_records_by_query};
use crate::repository::{ClipboardRecord, ClipboardRepository};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FilteredRecordsUpdate {
    None,
    Reset { new_len: usize },
    Splice { old_len: usize, new_len: usize },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FilteredRecordsSyncPlan {
    pub(super) indices: Vec<usize>,
    pub(super) selected_index: usize,
    pub(super) list_update: FilteredRecordsUpdate,
    pub(super) clear_deleting_record: bool,
}

pub(super) fn plan_filtered_records_sync(
    previous_indices: &[usize],
    next_indices: Vec<usize>,
    selected_index: usize,
    deleting_record: bool,
) -> FilteredRecordsSyncPlan {
    let list_update = if next_indices == previous_indices {
        FilteredRecordsUpdate::None
    } else if deleting_record {
        FilteredRecordsUpdate::Splice {
            old_len: previous_indices.len(),
            new_len: next_indices.len(),
        }
    } else {
        FilteredRecordsUpdate::Reset {
            new_len: next_indices.len(),
        }
    };

    let selected_index = if next_indices.is_empty() {
        0
    } else {
        selected_index.min(next_indices.len() - 1)
    };

    FilteredRecordsSyncPlan {
        indices: next_indices,
        selected_index,
        clear_deleting_record: deleting_record,
        list_update,
    }
}

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

#[cfg(test)]
mod tests {
    use super::{FilteredRecordsUpdate, plan_filtered_records_sync};

    #[test]
    fn test_plan_filtered_records_sync_uses_splice_for_delete_flow() {
        let plan = plan_filtered_records_sync(&[0, 1, 2], vec![0, 2], 2, true);

        assert_eq!(plan.indices, vec![0, 2]);
        assert_eq!(plan.selected_index, 1);
        assert_eq!(
            plan.list_update,
            FilteredRecordsUpdate::Splice {
                old_len: 3,
                new_len: 2,
            }
        );
        assert!(plan.clear_deleting_record);
    }

    #[test]
    fn test_plan_filtered_records_sync_uses_reset_for_filter_changes() {
        let plan = plan_filtered_records_sync(&[0, 1, 2], vec![1], 2, false);

        assert_eq!(plan.indices, vec![1]);
        assert_eq!(plan.selected_index, 0);
        assert_eq!(
            plan.list_update,
            FilteredRecordsUpdate::Reset { new_len: 1 }
        );
        assert!(!plan.clear_deleting_record);
    }

    #[test]
    fn test_plan_filtered_records_sync_is_noop_when_indices_unchanged() {
        let plan = plan_filtered_records_sync(&[1], vec![1], 0, false);

        assert_eq!(plan.indices, vec![1]);
        assert_eq!(plan.selected_index, 0);
        assert_eq!(plan.list_update, FilteredRecordsUpdate::None);
        assert!(!plan.clear_deleting_record);
    }
}

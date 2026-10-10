use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

use gpui_kit::{AppContext, Context, ReadGlobal};

use super::{
    RopyBoard,
    filtering::{
        ClearConfirmAction, FilteredRecordsUpdate, filter_and_sort_record_indices,
        plan_filtered_records_sync,
    },
    search::ContentFilter,
};

/// Serialize cleanup/query work without holding request state during I/O.
#[derive(Debug, Default)]
pub(super) struct HistoryRefresh {
    request: Mutex<HistoryRequest>,
    operation: Mutex<()>,
}

#[derive(Debug, Default)]
struct HistoryRequest {
    revision: u64,
    trim_to_limit: bool,
}

impl HistoryRefresh {
    fn next_revision(&self) -> u64 {
        self.request(false)
    }

    fn request(&self, trim_to_limit: bool) -> u64 {
        // A coalesced notification must not downgrade a storage-settings trim.
        let mut request = lock_or_recover(&self.request);
        request.trim_to_limit |= trim_to_limit;
        request.revision = request.revision.wrapping_add(1);
        request.revision
    }

    fn is_current(&self, revision: u64) -> bool {
        lock_or_recover(&self.request).revision == revision
    }

    fn run<T>(&self, revision: u64, operation: impl FnOnce(bool) -> T) -> Option<T> {
        let _operation = lock_or_recover(&self.operation);
        let trim_to_limit = {
            let mut request = lock_or_recover(&self.request);
            if request.revision != revision {
                return None;
            }
            std::mem::take(&mut request.trim_to_limit)
        };
        Some(operation(trim_to_limit))
    }
}

use crate::{
    clipboard::delete_tracked_record,
    gui::{repository::GlobalRepository, settings::GlobalSettings},
    utils::{lock_or_recover, read_or_recover, write_or_recover},
};

impl RopyBoard {
    pub(crate) fn sync_filtered_records(&mut self, cx: &Context<'_, Self>) {
        self.sync_filtered_records_internal(cx, false);
    }

    pub(crate) fn sync_filtered_records_and_reveal(&mut self, cx: &Context<'_, Self>) {
        self.sync_filtered_records_internal(cx, true);
    }

    fn sync_filtered_records_internal(&mut self, cx: &Context<'_, Self>, reveal_selection: bool) {
        let query = self.search_input.read(cx).value().to_string();
        let previous_visible_len = self.visible_list_len(self.filtered_record_indices.len());
        let next_indices = self.get_filtered_record_indices(&query);
        let plan = plan_filtered_records_sync(
            self.filtered_record_indices.as_ref(),
            next_indices,
            self.selected_index,
            self.ui_state.is_deleting_record(),
        );
        let next_visible_len = self.visible_list_len(plan.indices.len());

        let scroll_position = matches!(plan.list_update, FilteredRecordsUpdate::Splice { .. })
            .then(|| self.list_state.logical_scroll_top());

        self.filtered_record_indices = Arc::new(plan.indices);
        self.selected_index = plan.selected_index;

        match plan.list_update {
            FilteredRecordsUpdate::None => {}
            FilteredRecordsUpdate::Reset { .. } => {
                self.list_state.reset(next_visible_len);
            }
            FilteredRecordsUpdate::Splice { .. } => {
                self.list_state
                    .splice(0..previous_visible_len, next_visible_len);
                if let Some(scroll_position) = scroll_position {
                    self.list_state.scroll_to(scroll_position);
                }
            }
        }

        if plan.clear_deleting_record {
            self.ui_state.deletion = crate::gui::board::DeletionState::Idle;
        }

        if reveal_selection {
            self.reveal_selected_record();
        }
    }

    pub(crate) fn refresh_records_from_repository(&mut self, cx: &Context<'_, Self>) {
        self.refresh_history(cx, false);
    }

    pub(super) fn refresh_history(&mut self, cx: &Context<'_, Self>, trim_to_limit: bool) {
        let Some(repo) = GlobalRepository::global(cx).cloned() else {
            return;
        };
        let revision = self.history_refresh.request(trim_to_limit);
        let history_refresh = self.history_refresh.clone();
        let (max_history, max_storage) = GlobalSettings::read(cx, |settings| {
            (
                settings.storage.max_history_records,
                settings.storage.max_storage_records,
            )
        });
        let query = cx.background_spawn(async move {
            history_refresh.run(revision, |trim_to_limit| {
                let cleanup = if trim_to_limit {
                    repo.cleanup_old_records(max_storage)
                } else {
                    repo.cleanup_old_records_if_needed(max_storage)
                };
                if let Err(error) = cleanup {
                    tracing::warn!(%error, "failed to cleanup old clipboard records");
                }
                repo.get_display_snapshot(max_history)
            })
        });
        self.history_refresh_task = Some(cx.spawn(async move |board, cx| {
            let Some(result) = query.await else {
                return;
            };
            let _ = board.update(cx, |board, cx| {
                if !board.history_refresh.is_current(revision) {
                    return;
                }
                match result {
                    Ok(snapshot) => {
                        board.apply_history_snapshot(snapshot, cx);
                        cx.notify();
                    }
                    Err(error) => tracing::warn!(%error, "failed to reload display snapshot"),
                }
            });
        }));
    }

    fn apply_history_snapshot(
        &mut self,
        snapshot: crate::repository::DisplaySnapshot,
        cx: &Context<'_, Self>,
    ) {
        // Selection, scrolling and filters are read at completion, so interaction
        // during I/O is not undone by the request's earlier presentation state.
        let selected_id = self.filtered_record_id_at(self.selected_index);
        let scroll_position = self.list_state.logical_scroll_top();
        let (records, favorite_ids) = snapshot.into_parts();
        *write_or_recover(&self.records) = records;
        self.favorite_ids = Arc::new(favorite_ids);

        self.sync_filtered_records(cx);
        if let Some(index) = selected_id.and_then(|id| {
            let records = read_or_recover(&self.records);
            self.filtered_record_indices
                .iter()
                .position(|&index| records.get(index).is_some_and(|record| record.id == id))
        }) {
            self.selected_index = index;
        }
        // Record content and order may change while positional indices stay equal.
        self.list_state
            .reset(self.visible_list_len(self.filtered_record_len()));
        self.list_state.scroll_to(scroll_position);
    }

    /// Wipe everything — including pinned and favorited records — used by
    /// the "clear all" path. Most callers want
    /// [`Self::clear_ordinary_history`] instead.
    pub(crate) fn clear_history(&mut self, cx: &Context<'_, Self>) {
        GlobalRepository::read(cx, |repo| {
            if let Some(repo) = repo {
                if let Err(e) = repo.clear() {
                    tracing::warn!(error = %e, "failed to clear clipboard history");
                } else {
                    self.history_refresh.next_revision();
                    self.history_refresh_task = None;
                    {
                        let mut guard = write_or_recover(&self.records);
                        guard.clear();
                    }
                    self.favorite_ids = Arc::new(HashSet::new());
                    self.sync_filtered_records(cx);
                }
            }
        });
    }

    /// Default "clear history" path: pinned and favorited records survive.
    pub(crate) fn clear_ordinary_history(&mut self, cx: &Context<'_, Self>) {
        GlobalRepository::read(cx, |repo| {
            if let Some(repo) = repo {
                if let Err(e) = repo.clear_ordinary_records() {
                    tracing::warn!(error = %e, "failed to clear ordinary clipboard records");
                } else {
                    self.refresh_records_from_repository(cx);
                }
            }
        });
    }

    pub(in crate::gui) fn open_clear_confirm(
        &mut self,
        action: ClearConfirmAction,
        window: &mut gpui_kit::Window,
        cx: &mut Context<'_, Self>,
    ) {
        self.clear_confirm_action = action;
        self.ui_state.clear_confirm = crate::gui::board::ClearConfirmState::Visible;
        super::clear_confirm::open(action, window, cx);
        cx.notify();
    }

    pub(crate) fn confirm_clear_action(&mut self, cx: &Context<'_, Self>) {
        match self.clear_confirm_action {
            ClearConfirmAction::AllHistory => self.clear_history(cx),
            ClearConfirmAction::OrdinaryRecords => self.clear_ordinary_history(cx),
        }

        self.clear_last_copy_state();
    }

    /// Reset the dedup gate so the next copy — even if identical to the
    /// just-cleared content — is recaptured by the listener.
    pub(crate) fn clear_last_copy_state(&self) {
        let mut guard = lock_or_recover(&self.last_copy);
        guard.clear();
    }

    pub(crate) fn delete_record(&mut self, id: u64, cx: &Context<'_, Self>) {
        GlobalRepository::read(cx, |repo| {
            if let Some(repo) = repo {
                if let Err(e) = delete_tracked_record(repo, id, &self.last_copy) {
                    tracing::warn!(error = %e, "failed to delete clipboard record");
                } else {
                    self.ui_state.deletion = crate::gui::board::DeletionState::Deleting;
                    self.refresh_records_from_repository(cx);
                }
            }
        });
    }

    /// Returns true if the record is pinned or favorited (i.e. non-ordinary).
    fn is_record_special(&self, id: u64) -> bool {
        self.favorite_ids.contains(&id)
            || read_or_recover(&self.records)
                .iter()
                .any(|r| r.id == id && r.pinned)
    }

    /// Requests deletion of a record. If the record is non-ordinary (pinned or
    /// favorited), a confirmation dialog is shown first.
    /// Returns `true` when a confirmation dialog was shown (deletion deferred).
    pub(crate) fn request_delete_record(
        &mut self,
        id: u64,
        window: &mut gpui_kit::Window,
        cx: &mut Context<'_, Self>,
    ) -> bool {
        if self.is_record_special(id) {
            self.pending_delete_id = Some(id);
            self.ui_state.delete_confirm = crate::gui::board::DeleteConfirmState::Visible;
            super::delete_confirm::open(window, cx);
            cx.notify();
            true
        } else {
            self.delete_record(id, cx);
            false
        }
    }

    /// Confirms and executes the pending single-record deletion.
    pub(crate) fn confirm_pending_delete(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(id) = self.pending_delete_id.take() {
            self.delete_record(id, cx);
            self.reveal_selected_record();
        }
        self.ui_state.delete_confirm = crate::gui::board::DeleteConfirmState::Hidden;
        cx.notify();
    }

    /// Cancels the pending single-record delete confirmation.
    pub(crate) fn cancel_pending_delete(&mut self, cx: &mut Context<'_, Self>) {
        self.pending_delete_id = None;
        self.ui_state.delete_confirm = crate::gui::board::DeleteConfirmState::Hidden;
        cx.notify();
    }

    pub(crate) fn toggle_record_favorite(&mut self, id: u64, cx: &Context<'_, Self>) {
        GlobalRepository::read(cx, |repo| {
            let Some(repo) = repo else {
                return;
            };
            match repo.toggle_favorite(id) {
                Ok(_) => {
                    self.refresh_records_from_repository(cx);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to toggle favorite on clipboard record");
                }
            }
        });
    }

    pub(crate) fn toggle_record_pin(&mut self, id: u64, cx: &Context<'_, Self>) {
        GlobalRepository::read(cx, |repo| {
            let Some(repo) = repo else {
                return;
            };
            if let Err(e) = repo.toggle_pin(id) {
                tracing::warn!(error = %e, "failed to toggle pin on clipboard record");
                return;
            }
            self.refresh_records_from_repository(cx);
        });
    }

    /// Click-to-toggle behavior: re-clicking the active filter clears it
    /// (back to `All`) so the same button serves as both apply and reset.
    pub(crate) fn toggle_content_filter(&mut self, target: ContentFilter) {
        if self.filter_state.content_filter == target {
            self.filter_state.content_filter = ContentFilter::All;
        } else {
            self.filter_state.content_filter = target;
        }
    }

    /// Favorites toggle is intentionally orthogonal to the content filter
    /// so users can scope to e.g. "favorited images only".
    pub(crate) const fn toggle_favorites_only(&mut self) {
        self.filter_state.favorites_only = !self.filter_state.favorites_only;
    }

    pub(crate) const fn toggle_case_sensitive_search(&mut self) {
        self.filter_state.search_options.case_sensitive =
            !self.filter_state.search_options.case_sensitive;
    }

    pub(crate) const fn toggle_whole_word_search(&mut self) {
        self.filter_state.search_options.whole_word = !self.filter_state.search_options.whole_word;
    }

    pub(super) fn get_filtered_record_indices(&self, query: &str) -> Vec<usize> {
        let records = read_or_recover(&self.records);
        filter_and_sort_record_indices(
            &records,
            query,
            self.filter_state.content_filter,
            self.filter_state.search_options,
            &self.favorite_ids,
            self.filter_state.favorites_only,
        )
    }

    pub(crate) fn filtered_record_len(&self) -> usize {
        self.filtered_record_indices.len()
    }

    pub(crate) fn filtered_record_index_at(&self, index: usize) -> Option<usize> {
        self.filtered_record_indices.get(index).copied()
    }

    pub(crate) fn filtered_record_id_at(&self, index: usize) -> Option<u64> {
        let records = read_or_recover(&self.records);
        let record_index = self.filtered_record_index_at(index)?;
        records.get(record_index).map(|record| record.id)
    }
}

#[cfg(test)]
mod refresh_tests {
    use super::HistoryRefresh;

    #[test]
    fn test_history_refresh_request_during_io_keeps_new_trim_pending() {
        let refresh = HistoryRefresh::default();
        let old = refresh.request(true);
        let latest = refresh.run(old, |trim| {
            assert!(trim);
            refresh.request(true)
        });
        let latest = latest.unwrap_or_default();
        assert_eq!(refresh.run(latest, |trim| trim), Some(true));
        assert_eq!(
            refresh.run(refresh.request(false), |trim| trim),
            Some(false)
        );
    }

    #[test]
    fn test_history_refresh_superseded_cleanup_is_skipped_before_execution() {
        let refresh = HistoryRefresh::default();
        let old = refresh.next_revision();
        let latest = refresh.next_revision();
        let cleaned = std::cell::Cell::new(false);
        assert!(refresh.run(old, |_| cleaned.set(true)).is_none());
        assert!(!cleaned.get(), "obsolete limits must not delete history");
        assert!(refresh.run(latest, |_| cleaned.set(true)).is_some());
        assert!(cleaned.get());
    }
}

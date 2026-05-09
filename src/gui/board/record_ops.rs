use std::{collections::HashSet, sync::Arc};

use gpui::Context;

use super::{
    RopyBoard,
    filtering::{
        ClearConfirmAction, FilteredRecordsUpdate, filter_and_sort_record_indices,
        plan_filtered_records_sync,
    },
    search::ContentFilter,
};
use crate::{
    clipboard::LastCopyState,
    config::Settings,
    repository::GlobalRepository,
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

    pub(super) fn load_favorite_ids(cx: &gpui::App) -> HashSet<u64> {
        GlobalRepository::read(cx, |repo| {
            repo.and_then(|repo| repo.favorite_ids().ok())
                .map(|ids| ids.into_iter().collect())
                .unwrap_or_default()
        })
    }

    pub(crate) fn refresh_records_from_repository(&mut self, cx: &Context<'_, Self>) {
        let max_history_records = Settings::read(cx, |s| s.storage.max_history_records);

        GlobalRepository::read(cx, |repo| {
            let Some(repo) = repo else {
                return;
            };

            match repo.get_display_records(max_history_records) {
                Ok(records) => {
                    let mut guard = write_or_recover(&self.records);
                    *guard = records;
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to reload display records");
                }
            }

            match repo.favorite_ids() {
                Ok(ids) => {
                    self.favorite_ids = Arc::new(ids.into_iter().collect());
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to reload favorite ids");
                }
            }
        });

        self.sync_filtered_records(cx);
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
        cx: &mut Context<'_, Self>,
    ) {
        self.clear_confirm_action = action;
        self.ui_state.clear_confirm = crate::gui::board::ClearConfirmState::Visible;
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
        *guard = LastCopyState::Text(String::new());
    }

    pub(crate) fn delete_record(&mut self, id: u64, cx: &Context<'_, Self>) {
        GlobalRepository::read(cx, |repo| {
            if let Some(repo) = repo {
                if let Err(e) = repo.delete(id) {
                    tracing::warn!(error = %e, "failed to delete clipboard record");
                } else {
                    self.ui_state.deletion = crate::gui::board::DeletionState::Deleting;
                    self.refresh_records_from_repository(cx);
                }
            }
        });
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

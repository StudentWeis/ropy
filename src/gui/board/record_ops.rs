use std::{collections::HashSet, sync::Arc};

use gpui::Context;

use super::{
    RopyBoard,
    filtering::{ClearConfirmAction, filter_and_sort_record_indices},
    search::ContentFilter,
};
use crate::{
    clipboard::LastCopyState,
    config::Settings,
    repository::GlobalRepository,
    utils::{lock_or_recover, read_or_recover, write_or_recover},
};

impl RopyBoard {
    pub(super) fn load_favorite_ids(cx: &gpui::App) -> HashSet<u64> {
        GlobalRepository::read(cx, |repo| {
            repo.and_then(|repo| repo.favorite_ids().ok())
                .map(|ids| ids.into_iter().collect())
                .unwrap_or_default()
        })
    }

    pub(super) fn refresh_records_from_repository(&mut self, cx: &Context<Self>) {
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
    }

    /// Clear clipboard history
    pub(crate) fn clear_history(&mut self, cx: &Context<Self>) {
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
                }
            }
        });
    }

    /// Clear only ordinary clipboard history, preserving pinned and favorited records.
    pub(crate) fn clear_ordinary_history(&mut self, cx: &Context<Self>) {
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
        cx: &mut Context<Self>,
    ) {
        self.clear_confirm_action = action;
        self.show_clear_confirm = true;
        cx.notify();
    }

    pub(crate) fn confirm_clear_action(&mut self, cx: &Context<Self>) {
        match self.clear_confirm_action {
            ClearConfirmAction::AllHistory => self.clear_history(cx),
            ClearConfirmAction::OrdinaryRecords => self.clear_ordinary_history(cx),
        }

        self.clear_last_copy_state();
    }

    /// Clear last copy state
    pub(crate) fn clear_last_copy_state(&self) {
        let mut guard = lock_or_recover(&self.last_copy);
        *guard = LastCopyState::Text(String::new());
    }

    /// Delete a single record by ID
    pub fn delete_record(&mut self, id: u64, cx: &Context<Self>) {
        GlobalRepository::read(cx, |repo| {
            if let Some(repo) = repo {
                if let Err(e) = repo.delete(id) {
                    tracing::warn!(error = %e, "failed to delete clipboard record");
                } else {
                    self.deleting_record = true;
                    self.refresh_records_from_repository(cx);
                }
            }
        });
    }

    /// Toggle favorite state of a record.
    pub fn toggle_record_favorite(&mut self, id: u64, cx: &Context<Self>) {
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

    /// Toggle pin state of a record
    pub fn toggle_record_pin(&mut self, id: u64, cx: &Context<Self>) {
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

    /// Toggle the content type filter. Clicking the same filter again resets to All.
    pub(crate) fn toggle_content_filter(&mut self, target: ContentFilter) {
        if self.content_filter == target {
            self.content_filter = ContentFilter::All;
        } else {
            self.content_filter = target;
        }
    }

    /// Toggle the favorites-only filter (independent of content type filter).
    pub(crate) const fn toggle_favorites_only(&mut self) {
        self.favorites_only = !self.favorites_only;
    }

    pub(crate) const fn toggle_case_sensitive_search(&mut self) {
        self.search_options.case_sensitive = !self.search_options.case_sensitive;
    }

    pub(crate) const fn toggle_whole_word_search(&mut self) {
        self.search_options.whole_word = !self.search_options.whole_word;
    }

    /// Get filtered records based on search query and content type filter
    pub(super) fn get_filtered_record_indices(&self, query: &str) -> Vec<usize> {
        let records = read_or_recover(&self.records);
        filter_and_sort_record_indices(
            &records,
            query,
            self.content_filter,
            self.search_options,
            &self.favorite_ids,
            self.favorites_only,
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

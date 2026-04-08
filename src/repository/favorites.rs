//! Favorite record management for the clipboard repository.

use std::collections::HashSet;

use chrono::Local;

use super::{errors::RepositoryError, repo::ClipboardRepository};

impl ClipboardRepository {
    /// Return all favorite record IDs.
    pub fn favorite_ids(&self) -> Result<Vec<u64>, RepositoryError> {
        let mut ids = Vec::new();

        self.favorites.scan_ascending(&mut |key, _value| {
            if let Some(id) = Self::decode_u64_key(key) {
                ids.push(id);
            }
            true
        })?;

        ids.sort_unstable();
        Ok(ids)
    }

    /// Toggle the favorite state of a record.
    ///
    /// Returns the new favorite state after the operation.
    pub fn toggle_favorite(&self, id: u64) -> Result<bool, RepositoryError> {
        if self.get_by_id(id)?.is_none() {
            return Err(RepositoryError::Query("record not found".to_string()));
        }

        let key = id.to_be_bytes();
        if self.favorites.get(&key)?.is_some() {
            self.favorites.remove(&key)?;
            return Ok(false);
        }

        let favorited_at = Local::now().timestamp_millis().to_be_bytes();
        self.favorites.insert(&key, &favorited_at)?;
        Ok(true)
    }

    /// Remove a record from favorites.
    pub(super) fn remove_favorite(&self, id: u64) -> Result<(), RepositoryError> {
        self.favorites.remove(&id.to_be_bytes())?;
        Ok(())
    }

    /// Collect all favorite IDs into a `HashSet`, filtering out stale entries.
    pub(super) fn favorite_id_set(&self) -> Result<HashSet<u64>, RepositoryError> {
        let mut favorite_ids = HashSet::new();

        for id in self.favorite_ids()? {
            if self.get_raw(&id.to_be_bytes())?.is_some() {
                favorite_ids.insert(id);
            }
        }

        Ok(favorite_ids)
    }
}

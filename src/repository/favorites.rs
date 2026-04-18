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

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use crate::repository::test_helpers::create_test_repo;

    #[test]
    #[allow(clippy::expect_used)]
    fn test_favorite_ids_when_empty_returns_empty_vec() {
        let repo = create_test_repo();

        let ids = repo.favorite_ids().expect("Failed to get favorite ids");

        assert!(ids.is_empty());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_favorite_ids_returns_sorted_ids() {
        let repo = create_test_repo();

        let record_a = repo.save_text("A".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let record_b = repo.save_text("B".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let record_c = repo.save_text("C".to_string()).expect("Failed to save");

        // Favorite in non-sorted order
        repo.toggle_favorite(record_c.id)
            .expect("Failed to favorite");
        repo.toggle_favorite(record_a.id)
            .expect("Failed to favorite");
        repo.toggle_favorite(record_b.id)
            .expect("Failed to favorite");

        let ids = repo.favorite_ids().expect("Failed to get favorite ids");

        assert_eq!(ids.len(), 3);
        // IDs should be sorted in ascending order
        assert!(ids.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_toggle_favorite_when_record_not_found_returns_error() {
        let repo = create_test_repo();

        let result = repo.toggle_favorite(999);

        assert!(result.is_err());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_toggle_favorite_twice_returns_to_unfavorited() {
        let repo = create_test_repo();

        let record = repo
            .save_text("Toggle me".to_string())
            .expect("Failed to save");

        let first_toggle = repo.toggle_favorite(record.id).expect("Failed to toggle");
        assert!(first_toggle, "first toggle should favorite");

        let second_toggle = repo.toggle_favorite(record.id).expect("Failed to toggle");
        assert!(!second_toggle, "second toggle should unfavorite");

        let ids = repo.favorite_ids().expect("Failed to get favorite ids");
        assert!(ids.is_empty());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_favorite_id_set_filters_stale_entries() {
        let repo = create_test_repo();

        let record_a = repo.save_text("A".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let record_b = repo.save_text("B".to_string()).expect("Failed to save");

        repo.toggle_favorite(record_a.id)
            .expect("Failed to favorite");
        repo.toggle_favorite(record_b.id)
            .expect("Failed to favorite");

        // Delete record_a from the records store, making its favorite entry stale
        repo.delete(record_a.id).expect("Failed to delete");

        let favorite_set = repo.favorite_id_set().expect("Failed to get favorite set");

        // Only record_b should remain since record_a's data was deleted
        assert_eq!(favorite_set.len(), 1);
        assert!(favorite_set.contains(&record_b.id));
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_remove_favorite_removes_entry() {
        let repo = create_test_repo();

        let record = repo
            .save_text("Removable".to_string())
            .expect("Failed to save");
        repo.toggle_favorite(record.id).expect("Failed to favorite");

        assert_eq!(repo.favorite_ids().expect("query").len(), 1);

        repo.remove_favorite(record.id)
            .expect("Failed to remove favorite");

        assert!(repo.favorite_ids().expect("query").is_empty());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_remove_favorite_when_not_favorited_succeeds_silently() {
        let repo = create_test_repo();

        let record = repo
            .save_text("Not favorited".to_string())
            .expect("Failed to save");

        // Should not error even though the record is not favorited
        repo.remove_favorite(record.id)
            .expect("Failed to remove favorite");

        assert!(repo.favorite_ids().expect("query").is_empty());
    }
}

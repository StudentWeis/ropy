//! Clipboard repository for storing and retrieving clipboard records.

use chrono::Local;
use sled::{Db, Tree};
use std::path::PathBuf;

use super::errors::RepositoryError;
use super::models::{ClipboardRecord, ContentType};

pub struct ClipboardRepository {
    db: Db,
    records_tree: Tree,
}

impl ClipboardRepository {
    /// Create a new repository instance
    pub fn new() -> Result<Self, RepositoryError> {
        // The database file is stored in the user data directory at `ropy/clipboard.db`
        let data_dir = Self::get_data_dir()?;
        Self::with_path(data_dir)
    }

    /// Create a repository with a specific database file path
    pub fn with_path(path: PathBuf) -> Result<Self, RepositoryError> {
        let db = sled::open(&path).map_err(|e| RepositoryError::DatabaseOpen(e.to_string()))?;
        let records_tree = db
            .open_tree("clipboard_records")
            .map_err(|e| RepositoryError::TreeOpen(e.to_string()))?;

        Ok(Self { db, records_tree })
    }

    /// Get the data directory path for storing the database file
    fn get_data_dir() -> Result<PathBuf, RepositoryError> {
        let data_dir = dirs::data_local_dir()
            .ok_or(RepositoryError::DataDirNotFound)?
            .join("ropy")
            .join("clipboard.db");
        Ok(data_dir)
    }

    /// Save a clipboard record
    ///
    /// Uses a timestamp as the key to ensure chronological storage
    pub fn save(
        &self,
        content: String,
        content_type: ContentType,
    ) -> Result<ClipboardRecord, RepositoryError> {
        let now = Local::now();
        let id = now.timestamp_nanos_opt().unwrap_or(0) as u64;

        let record = ClipboardRecord {
            id,
            content,
            created_at: now,
            content_type,
        };

        let key = id.to_be_bytes();
        let value = serde_json::to_vec(&record)
            .map_err(|e| RepositoryError::Serialization(e.to_string()))?;

        self.records_tree
            .insert(key, value)
            .map_err(|e| RepositoryError::Insert(e.to_string()))?;

        Ok(record)
    }

    /// Save text content (convenience method)
    pub fn save_text(&self, content: String) -> Result<ClipboardRecord, RepositoryError> {
        self.save(content, ContentType::Text)
    }

    /// Check if the content is duplicate of the latest record
    pub fn is_duplicate(&self, content: &str) -> Result<bool, RepositoryError> {
        if let Some(last_record) = self.get_latest()? {
            return Ok(last_record.content == content);
        }
        Ok(false)
    }

    /// Save text content if it is not a duplicate of the latest record
    pub fn save_text_if_not_duplicate(
        &self,
        content: String,
    ) -> Result<Option<ClipboardRecord>, RepositoryError> {
        if self.is_duplicate(&content)? {
            return Ok(None);
        }
        self.save_text(content).map(Some)
    }

    /// Get the latest record
    pub fn get_latest(&self) -> Result<Option<ClipboardRecord>, RepositoryError> {
        if let Some(result) = self
            .records_tree
            .last()
            .map_err(|e| RepositoryError::Query(e.to_string()))?
        {
            let (_, value) = result;
            let record: ClipboardRecord = serde_json::from_slice(&value)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            return Ok(Some(record));
        }
        Ok(None)
    }

    /// Get a record by ID
    #[allow(dead_code)]
    pub fn get_by_id(&self, id: u64) -> Result<Option<ClipboardRecord>, RepositoryError> {
        let key = id.to_be_bytes();
        if let Some(value) = self
            .records_tree
            .get(key)
            .map_err(|e| RepositoryError::Query(e.to_string()))?
        {
            let record: ClipboardRecord = serde_json::from_slice(&value)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            return Ok(Some(record));
        }
        Ok(None)
    }

    /// Get recent N records (in reverse chronological order)
    pub fn get_recent(&self, limit: usize) -> Result<Vec<ClipboardRecord>, RepositoryError> {
        let mut records = Vec::new();
        for result in self.records_tree.iter().rev().take(limit) {
            let (_, value) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
            let record: ClipboardRecord = serde_json::from_slice(&value)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            records.push(record);
        }
        Ok(records)
    }

    /// Search records by keyword
    pub fn search(&self, keyword: &str) -> Result<Vec<ClipboardRecord>, RepositoryError> {
        let keyword_lower = keyword.to_lowercase();
        let mut records = Vec::new();
        for result in self.records_tree.iter().rev() {
            let (_, value) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
            let record: ClipboardRecord = serde_json::from_slice(&value)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            if record.content.to_lowercase().contains(&keyword_lower) {
                records.push(record);
            }
        }
        Ok(records)
    }

    /// Delete a record
    #[allow(dead_code)]
    pub fn delete(&self, id: u64) -> Result<bool, RepositoryError> {
        let key = id.to_be_bytes();
        let removed = self
            .records_tree
            .remove(key)
            .map_err(|e| RepositoryError::Delete(e.to_string()))?;
        Ok(removed.is_some())
    }

    /// Clear all records
    pub fn clear(&self) -> Result<(), RepositoryError> {
        self.records_tree
            .clear()
            .map_err(|e| RepositoryError::Delete(e.to_string()))?;
        Ok(())
    }

    /// Get the total number of records
    pub fn count(&self) -> usize {
        self.records_tree.len()
    }

    /// Flush data to disk
    pub fn flush(&self) -> Result<(), RepositoryError> {
        self.db
            .flush()
            .map_err(|e| RepositoryError::Flush(e.to_string()))?;
        Ok(())
    }

    /// Clean up old records, keeping the most recent N records
    pub fn cleanup_old_records(&self, keep_count: usize) -> Result<usize, RepositoryError> {
        let total = self.count();
        if total <= keep_count {
            return Ok(0);
        }

        let to_remove = total - keep_count;
        let mut removed = 0;

        for result in self.records_tree.iter().take(to_remove) {
            let (key, _) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
            self.records_tree
                .remove(key)
                .map_err(|e| RepositoryError::Delete(e.to_string()))?;
            removed += 1;
        }

        Ok(removed)
    }
}

impl Drop for ClipboardRepository {
    fn drop(&mut self) {
        self.flush().ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;

    fn create_test_repo() -> ClipboardRepository {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        ClipboardRepository::with_path(db_path).expect("Failed to create test repository")
    }

    #[test]
    fn test_save_and_get_text() {
        let repo = create_test_repo();

        let record = repo
            .save_text("Hello, World!".to_string())
            .expect("Failed to save");
        assert_eq!(record.content, "Hello, World!");
        assert_eq!(record.content_type, ContentType::Text);

        let retrieved = repo
            .get_by_id(record.id)
            .expect("Failed to get by id")
            .expect("Record not found");
        assert_eq!(retrieved.content, "Hello, World!");
    }

    #[test]
    fn test_get_latest() {
        let repo = create_test_repo();

        repo.save_text("First".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("Second".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("Third".to_string()).expect("Failed to save");
        let latest = repo
            .get_latest()
            .expect("Failed to get latest")
            .expect("Record not found");
        assert_eq!(latest.content, "Third");
    }

    #[test]
    fn test_duplicate_check() {
        let repo = create_test_repo();

        repo.save_text("Same content".to_string())
            .expect("Failed to save");

        assert!(repo.is_duplicate("Same content").expect("Failed to check"));
        assert!(
            !repo
                .is_duplicate("Different content")
                .expect("Failed to check")
        );
    }

    #[test]
    fn test_save_if_not_duplicate() {
        let repo = create_test_repo();

        let first = repo
            .save_text_if_not_duplicate("Content".to_string())
            .expect("Failed to save");
        assert!(first.is_some());

        let second = repo
            .save_text_if_not_duplicate("Content".to_string())
            .expect("Failed to save");
        assert!(second.is_none());

        let third = repo
            .save_text_if_not_duplicate("New Content".to_string())
            .expect("Failed to save");
        assert!(third.is_some());

        assert_eq!(repo.count(), 2);
    }

    #[test]
    fn test_get_recent() {
        let repo = create_test_repo();

        for i in 1..=5 {
            repo.save_text(format!("Record {}", i))
                .expect("Failed to save");
            thread::sleep(Duration::from_millis(10));
        }

        let recent = repo.get_recent(3).expect("Failed to get recent");
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].content, "Record 5");
        assert_eq!(recent[1].content, "Record 4");
        assert_eq!(recent[2].content, "Record 3");
    }

    #[test]
    fn test_search() {
        let repo = create_test_repo();

        repo.save_text("Hello World".to_string())
            .expect("Failed to save");
        repo.save_text("Goodbye World".to_string())
            .expect("Failed to save");
        repo.save_text("Hello Rust".to_string())
            .expect("Failed to save");

        let results = repo.search("hello").expect("Failed to search");
        assert_eq!(results.len(), 2);

        let results = repo.search("world").expect("Failed to search");
        assert_eq!(results.len(), 2);

        let results = repo.search("rust").expect("Failed to search");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_delete() {
        let repo = create_test_repo();

        let record = repo
            .save_text("To be deleted".to_string())
            .expect("Failed to save");
        assert_eq!(repo.count(), 1);

        let deleted = repo.delete(record.id).expect("Failed to delete");
        assert!(deleted);
        assert_eq!(repo.count(), 0);

        let deleted_again = repo.delete(record.id).expect("Failed to delete");
        assert!(!deleted_again);
    }

    #[test]
    fn test_clear() {
        let repo = create_test_repo();

        repo.save_text("One".to_string()).expect("Failed to save");
        repo.save_text("Two".to_string()).expect("Failed to save");
        repo.save_text("Three".to_string()).expect("Failed to save");
        assert_eq!(repo.count(), 3);

        repo.clear().expect("Failed to clear");
        assert_eq!(repo.count(), 0);
    }

    #[test]
    fn test_cleanup_old_records() {
        let repo = create_test_repo();

        for i in 1..=10 {
            repo.save_text(format!("Record {}", i))
                .expect("Failed to save");
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(repo.count(), 10);

        let removed = repo.cleanup_old_records(5).expect("Failed to clean up");
        assert_eq!(removed, 5);
        assert_eq!(repo.count(), 5);

        // Verify that the latest records are retained
        let recent = repo.get_recent(5).expect("Failed to get recent");
        assert_eq!(recent[0].content, "Record 10");
        assert_eq!(recent[4].content, "Record 6");
    }
}

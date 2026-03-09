//! Clipboard repository for storing and retrieving clipboard records.

use std::{fs, path::PathBuf};

use chrono::Local;
use sled::{Db, Tree};

use super::{
    errors::RepositoryError,
    models::{ClipboardRecord, ContentType},
};

/// Compute a deterministic content hash using seahash.
/// The content type is encoded as a prefix byte to avoid collisions
/// between different types with the same content.
fn content_hash(content: &str, content_type: &ContentType) -> u64 {
    let type_tag: u8 = match content_type {
        ContentType::Text => 0,
        ContentType::Image => 1,
        ContentType::FilePath => 2,
    };
    let mut data = vec![type_tag];
    data.extend_from_slice(content.as_bytes());
    seahash::hash(&data)
}

/// Schema version for the database. Bump this when the key format changes.
const SCHEMA_VERSION: u64 = 2;

pub struct ClipboardRepository {
    db: Db,
    records_tree: Tree,
    images_dir: PathBuf,
}

impl ClipboardRepository {
    /// Create a new repository instance
    pub fn new() -> Result<Self, RepositoryError> {
        // The database file is stored in the user data directory at `ropy/clipboard.db`
        let db_path = Self::get_db_path()?;
        let images_dir = dirs::data_local_dir()
            .ok_or(RepositoryError::DataDirNotFound)?
            .join("ropy")
            .join("images");
        Self::init(&db_path, images_dir)
    }

    /// Initialize repository with specific paths
    pub fn init(db_path: &PathBuf, images_dir: PathBuf) -> Result<Self, RepositoryError> {
        let db = sled::open(db_path).map_err(|e| RepositoryError::DatabaseOpen(e.to_string()))?;

        // Check schema version and clear old data if mismatched
        let meta_tree = db
            .open_tree("meta")
            .map_err(|e| RepositoryError::TreeOpen(e.to_string()))?;
        let version_key = b"schema_version";
        let needs_clear = match meta_tree
            .get(version_key)
            .map_err(|e| RepositoryError::Query(e.to_string()))?
        {
            Some(v) if v.len() == 8 => {
                let stored =
                    u64::from_be_bytes(v.as_ref().try_into().map_err(|_| {
                        RepositoryError::Deserialization("bad schema version".into())
                    })?);
                stored != SCHEMA_VERSION
            }
            _ => true,
        };

        let records_tree = db
            .open_tree("clipboard_records")
            .map_err(|e| RepositoryError::TreeOpen(e.to_string()))?;

        if needs_clear {
            records_tree
                .clear()
                .map_err(|e| RepositoryError::Delete(e.to_string()))?;
            // Clear old image files
            if images_dir.exists() {
                fs::remove_dir_all(&images_dir).ok();
            }
            meta_tree
                .insert(version_key, &SCHEMA_VERSION.to_be_bytes())
                .map_err(|e| RepositoryError::Insert(e.to_string()))?;
            db.flush()
                .map_err(|e| RepositoryError::Flush(e.to_string()))?;
        }

        Ok(Self {
            db,
            records_tree,
            images_dir,
        })
    }

    /// Get the data directory path for storing the database file
    fn get_db_path() -> Result<PathBuf, RepositoryError> {
        let data_dir = dirs::data_local_dir()
            .ok_or(RepositoryError::DataDirNotFound)?
            .join("ropy")
            .join("clipboard.db");
        Ok(data_dir)
    }

    /// Save a clipboard record
    ///
    /// Uses content hash as the key for deduplication.
    /// If a record with the same content already exists, only `created_at` is updated.
    pub fn save(
        &self,
        content: String,
        content_type: ContentType,
    ) -> Result<ClipboardRecord, RepositoryError> {
        let id = content_hash(&content, &content_type);
        let key = id.to_be_bytes();
        let now = Local::now();

        // Check if a record with the same hash already exists
        if let Some(existing) = self
            .records_tree
            .get(key)
            .map_err(|e| RepositoryError::Query(e.to_string()))?
        {
            let mut record: ClipboardRecord = serde_json::from_slice(&existing)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            record.created_at = now;
            let value = serde_json::to_vec(&record)
                .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
            self.records_tree
                .insert(key, value)
                .map_err(|e| RepositoryError::Insert(e.to_string()))?;
            return Ok(record);
        }

        let record = ClipboardRecord {
            id,
            content,
            created_at: now,
            content_type,
            pinned: false,
        };

        let value = serde_json::to_vec(&record)
            .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
        self.records_tree
            .insert(key, value)
            .map_err(|e| RepositoryError::Insert(e.to_string()))?;

        Ok(record)
    }

    /// Save image record from existing file path
    ///
    /// Uses the provided `content_hash` (computed from image bytes) as the key
    /// for deduplication. When a duplicate is found the newly saved image file
    /// is removed and only `created_at` is updated on the existing record.
    pub fn save_image_from_path(
        &self,
        file_path: String,
        image_content_hash: u64,
    ) -> Result<ClipboardRecord, RepositoryError> {
        let id = image_content_hash;
        let key = id.to_be_bytes();
        let now = Local::now();

        // Check if a record with the same image hash already exists
        if let Some(existing) = self
            .records_tree
            .get(key)
            .map_err(|e| RepositoryError::Query(e.to_string()))?
        {
            let mut record: ClipboardRecord = serde_json::from_slice(&existing)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;

            // If paths differ, delete the newly generated duplicate image file
            if record.content != file_path {
                let _ = fs::remove_file(&file_path);
                let thumb_path = file_path.replace(".png", "_thumb.png");
                let _ = fs::remove_file(thumb_path);
            }

            record.created_at = now;
            let value = serde_json::to_vec(&record)
                .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
            self.records_tree
                .insert(key, value)
                .map_err(|e| RepositoryError::Insert(e.to_string()))?;
            return Ok(record);
        }

        let record = ClipboardRecord {
            id,
            content: file_path,
            created_at: now,
            content_type: ContentType::Image,
            pinned: false,
        };

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

    /// Get a record by ID
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
    ///
    /// Pinned records are always displayed first, followed by unpinned
    /// records. Records that fail to deserialize (e.g. old format) are
    /// silently skipped.
    pub fn get_recent(&self, limit: usize) -> Result<Vec<ClipboardRecord>, RepositoryError> {
        let mut records = Vec::new();
        for result in &self.records_tree {
            let (_, value) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
            match serde_json::from_slice::<ClipboardRecord>(&value) {
                Ok(record) => records.push(record),
                Err(e) => {
                    tracing::warn!(error = %e, "skipping record that failed to deserialize");
                }
            }
        }
        Self::sort_pinned_first(&mut records);
        records.truncate(limit);
        Ok(records)
    }

    /// Search records by keyword
    ///
    /// Records that fail to deserialize (e.g. old format) are silently
    /// skipped rather than causing the entire search to fail.
    pub fn search(&self, keyword: &str) -> Result<Vec<ClipboardRecord>, RepositoryError> {
        let keyword_lower = keyword.to_lowercase();
        let mut records = Vec::new();
        for result in &self.records_tree {
            let (_, value) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
            let record: ClipboardRecord = match serde_json::from_slice(&value) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(error = %e, "skipping record that failed to deserialize");
                    continue;
                }
            };
            // Only search in text records
            if record.content_type == ContentType::Text
                && record.content.to_lowercase().contains(&keyword_lower)
            {
                records.push(record);
            }
        }
        Self::sort_pinned_first(&mut records);
        Ok(records)
    }

    /// Sort records so that pinned items appear first and both groups remain
    /// ordered by descending creation time.
    pub(crate) fn sort_pinned_first(records: &mut [ClipboardRecord]) {
        records.sort_unstable_by(|a, b| match (a.pinned, b.pinned) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.created_at.cmp(&a.created_at),
        });
    }

    /// Toggle the pin state of a record
    pub fn toggle_pin(&self, id: u64) -> Result<(), RepositoryError> {
        let mut record = self
            .get_by_id(id)?
            .ok_or_else(|| RepositoryError::Query("record not found".to_string()))?;

        record.pinned = !record.pinned;

        let key = id.to_be_bytes();
        let value = serde_json::to_vec(&record)
            .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
        self.records_tree
            .insert(key, value)
            .map_err(|e| RepositoryError::Insert(e.to_string()))?;

        Ok(())
    }

    /// Delete a record
    pub fn delete(&self, id: u64) -> Result<bool, RepositoryError> {
        // If it's an image record, delete the associated image file
        let record = self.get_by_id(id)?;
        if let Some(rec) = record
            && rec.content_type == ContentType::Image
        {
            // Delete original image file and thumbnail
            let _ = fs::remove_file(&rec.content);
            let thumb_path = rec.content.replace(".png", "_thumb.png");
            let _ = fs::remove_file(thumb_path);
        }
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
        // Clear all image files
        if self.images_dir.exists() {
            fs::remove_dir_all(&self.images_dir).ok();
        }
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
    ///
    /// Pinned records are never removed during cleanup.
    pub fn cleanup_old_records(&self, keep_count: usize) -> Result<usize, RepositoryError> {
        let total = self.count();
        if total <= keep_count {
            return Ok(0);
        }

        let mut records: Vec<(u64, chrono::DateTime<Local>, bool)> = Vec::new();
        for result in &self.records_tree {
            let (_, value) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
            let record: ClipboardRecord = serde_json::from_slice(&value)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            records.push((record.id, record.created_at, record.pinned));
        }
        // Sort by created_at ascending so the oldest come first
        records.sort_unstable_by(|a, b| a.1.cmp(&b.1));

        let to_remove = total - keep_count;
        let mut removed = 0;
        for (id, _, is_pinned) in records {
            if removed >= to_remove {
                break;
            }
            if is_pinned {
                continue;
            }
            let key = id.to_be_bytes();
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
    use std::{thread, time::Duration};

    use tempfile::tempdir;

    use super::*;

    #[allow(clippy::expect_used)]
    fn create_test_repo() -> ClipboardRepository {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        ClipboardRepository::init(&db_path, temp_dir.path().join("images"))
            .expect("Failed to create test repository")
    }

    #[test]
    #[allow(clippy::expect_used)]
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
    #[allow(clippy::expect_used)]
    fn test_get_recent() {
        let repo = create_test_repo();

        for i in 1..=5 {
            repo.save_text(format!("Record {i}"))
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
    #[allow(clippy::expect_used)]
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
    #[allow(clippy::expect_used)]
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
    #[allow(clippy::expect_used)]
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
    #[allow(clippy::expect_used)]
    fn test_cleanup_old_records() {
        let repo = create_test_repo();

        for i in 1..=10 {
            repo.save_text(format!("Record {i}"))
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

    #[test]
    #[allow(clippy::expect_used)]
    fn test_dedup_same_content() {
        let repo = create_test_repo();

        let r1 = repo
            .save_text("duplicate".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let r2 = repo
            .save_text("duplicate".to_string())
            .expect("Failed to save");

        // Same content produces the same id (content hash)
        assert_eq!(r1.id, r2.id);
        // Only one record in the database
        assert_eq!(repo.count(), 1);
        // created_at was updated
        assert!(r2.created_at > r1.created_at);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_dedup_aba_pattern() {
        let repo = create_test_repo();

        repo.save_text("A".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("B".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let a2 = repo.save_text("A".to_string()).expect("Failed to save");

        // Only 2 unique records (A and B)
        assert_eq!(repo.count(), 2);
        // A is now the most recent
        let recent = repo.get_recent(2).expect("Failed to get recent");
        assert_eq!(recent[0].content, "A");
        assert_eq!(recent[0].created_at, a2.created_at);
        assert_eq!(recent[1].content, "B");
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_content_hash_deterministic() {
        // Verify that the same content always maps to the same id
        let repo = create_test_repo();

        let r1 = repo
            .save_text("stable hash".to_string())
            .expect("Failed to save");
        let expected_id = r1.id;

        // Save again — should return the same id
        let r2 = repo
            .save_text("stable hash".to_string())
            .expect("Failed to save");
        assert_eq!(r2.id, expected_id);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_toggle_pin() {
        let repo = create_test_repo();
        let record = repo
            .save_text("Pin me".to_string())
            .expect("Failed to save");

        assert!(!record.pinned);

        repo.toggle_pin(record.id).expect("Failed to toggle pin");
        let pinned = repo
            .get_by_id(record.id)
            .expect("Failed to get")
            .expect("not found");
        assert!(pinned.pinned);

        repo.toggle_pin(record.id).expect("Failed to toggle pin");
        let unpinned = repo
            .get_by_id(record.id)
            .expect("Failed to get")
            .expect("not found");
        assert!(!unpinned.pinned);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_pinned_records_appear_first() {
        let repo = create_test_repo();

        repo.save_text("First".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let second = repo
            .save_text("Second".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("Third".to_string()).expect("Failed to save");

        // Pin the second record
        repo.toggle_pin(second.id).expect("Failed to toggle pin");

        let recent = repo.get_recent(10).expect("Failed to get recent");
        assert_eq!(recent[0].content, "Second"); // pinned → first
        assert_eq!(recent[1].content, "Third");
        assert_eq!(recent[2].content, "First");
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_multiple_pinned_ordering() {
        let repo = create_test_repo();

        let r1 = repo.save_text("Alpha".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("Beta".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let r3 = repo.save_text("Gamma".to_string()).expect("Failed to save");

        repo.toggle_pin(r1.id).expect("Failed to toggle pin");
        repo.toggle_pin(r3.id).expect("Failed to toggle pin");

        let recent = repo.get_recent(10).expect("Failed to get recent");
        // Both pinned, newer first
        assert_eq!(recent[0].content, "Gamma");
        assert_eq!(recent[1].content, "Alpha");
        // Unpinned
        assert_eq!(recent[2].content, "Beta");
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_pinned_search() {
        let repo = create_test_repo();

        repo.save_text("hello world".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let r2 = repo
            .save_text("hello rust".to_string())
            .expect("Failed to save");

        repo.toggle_pin(r2.id).expect("Failed to toggle pin");

        let results = repo.search("hello").expect("Failed to search");
        assert_eq!(results.len(), 2);
        // Pinned result appears first
        assert_eq!(results[0].content, "hello rust");
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_cleanup_skips_pinned() {
        let repo = create_test_repo();

        let r1 = repo
            .save_text("Old pinned".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        for i in 2..=6 {
            repo.save_text(format!("Record {i}"))
                .expect("Failed to save");
            thread::sleep(Duration::from_millis(10));
        }

        repo.toggle_pin(r1.id).expect("Failed to toggle pin");
        assert_eq!(repo.count(), 6);

        let removed = repo.cleanup_old_records(3).expect("Failed to clean up");
        // The pinned record survives, and cleanup continues removing
        // unpinned records until the total count reaches the keep limit.
        assert_eq!(removed, 3);
        assert_eq!(repo.count(), 3);
        // Pinned record should survive
        let pinned = repo
            .get_by_id(r1.id)
            .expect("Failed to get")
            .expect("Pinned record should still exist");
        assert!(pinned.pinned);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_backward_compat_old_category_fields() {
        // Simulate a record stored with old `category` field
        let repo = create_test_repo();
        let old_json = serde_json::json!({
            "id": 1000_u64,
            "content": "legacy record",
            "created_at": chrono::Local::now(),
            "content_type": "Text",
            "category": "Pinned"
        });
        let key = 1000_u64.to_be_bytes();
        let value = serde_json::to_vec(&old_json).expect("failed to serialize");
        repo.records_tree
            .insert(key, value)
            .expect("failed to insert");

        // get_recent should deserialize it with default pinned = false
        let records = repo.get_recent(10).expect("Failed to get recent");
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].content, "legacy record");
        assert!(!records[0].pinned);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_cleanup_keeps_pinned_when_not_enough_unpinned() {
        let repo = create_test_repo();

        // Create 4 records, pin 3 of them
        let r1 = repo.save_text("A".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let r2 = repo.save_text("B".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let r3 = repo.save_text("C".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("D".to_string()).expect("Failed to save");

        repo.toggle_pin(r1.id).expect("Failed to toggle pin");
        repo.toggle_pin(r2.id).expect("Failed to toggle pin");
        repo.toggle_pin(r3.id).expect("Failed to toggle pin");

        // keep_count = 2, but 3 pinned records cannot be removed
        let removed = repo.cleanup_old_records(2).expect("Failed to clean up");
        // Only the 1 unpinned record (D) can be removed
        assert_eq!(removed, 1);
        // Total count = 3 (all pinned), which is above keep_count
        assert_eq!(repo.count(), 3);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_search_skips_corrupt_records() {
        let repo = create_test_repo();

        // Insert a valid record
        repo.save_text("valid hello".to_string())
            .expect("Failed to save");

        // Insert corrupt data
        let corrupt_key = 9999_u64.to_be_bytes();
        repo.records_tree
            .insert(corrupt_key, b"not valid json")
            .expect("failed to insert corrupt");

        // Search should still return the valid record
        let results = repo.search("hello").expect("search should not fail");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].content, "valid hello");

        // get_recent should also work
        let recent = repo.get_recent(10).expect("get_recent should not fail");
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].content, "valid hello");
    }
}

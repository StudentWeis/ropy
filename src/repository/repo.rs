//! Clipboard repository for storing and retrieving clipboard records.

use std::{fs, path::PathBuf};

use chrono::Local;
use sled::Db;

use super::{
    errors::RepositoryError,
    models::{ClipboardRecord, ContentType},
    time_index::TimeIndex,
};

/// Compute a deterministic content hash using seahash.
/// The content type is encoded as a prefix byte to avoid collisions
/// between different types with the same content.
fn content_hash(content: &str, content_type: &ContentType) -> u64 {
    let type_tag = content_type.as_tag();
    let mut data = vec![type_tag];
    data.extend_from_slice(content.as_bytes());
    seahash::hash(&data)
}

/// Schema version for the database. Bump this when the key format changes.
const SCHEMA_VERSION: u64 = 3;

pub struct ClipboardRepository {
    db: Db,
    records: sled::Tree,
    time_index: TimeIndex,
    images_dir: PathBuf,
}

impl ClipboardRepository {
    /// Create a new repository using the default data directory.
    pub fn new() -> Result<Self, RepositoryError> {
        let db_path = Self::default_db_path()?;
        let images_dir = dirs::data_local_dir()
            .ok_or(RepositoryError::DataDirNotFound)?
            .join("ropy")
            .join("images");
        Self::init(&db_path, images_dir)
    }

    /// Initialize repository with explicit paths (used by tests).
    pub fn init(db_path: &PathBuf, images_dir: PathBuf) -> Result<Self, RepositoryError> {
        let db = sled::open(db_path).map_err(|e| RepositoryError::DatabaseOpen(e.to_string()))?;

        let meta = db
            .open_tree("meta")
            .map_err(|e| RepositoryError::TreeOpen(e.to_string()))?;
        let records = db
            .open_tree("clipboard_records")
            .map_err(|e| RepositoryError::TreeOpen(e.to_string()))?;
        let time_index = TimeIndex::new(
            db.open_tree("time_index")
                .map_err(|e| RepositoryError::TreeOpen(e.to_string()))?,
        );

        if Self::needs_schema_migration(&meta)? {
            records
                .clear()
                .map_err(|e| RepositoryError::Delete(e.to_string()))?;
            time_index.clear()?;
            if images_dir.exists() {
                fs::remove_dir_all(&images_dir).ok();
            }
            meta.insert(b"schema_version", &SCHEMA_VERSION.to_be_bytes())
                .map_err(|e| RepositoryError::Insert(e.to_string()))?;
            db.flush()
                .map_err(|e| RepositoryError::Flush(e.to_string()))?;
        }

        Ok(Self {
            db,
            records,
            time_index,
            images_dir,
        })
    }

    /// Flush data to disk.
    pub fn flush(&self) -> Result<(), RepositoryError> {
        self.db
            .flush()
            .map_err(|e| RepositoryError::Flush(e.to_string()))?;
        Ok(())
    }

    fn default_db_path() -> Result<PathBuf, RepositoryError> {
        Ok(dirs::data_local_dir()
            .ok_or(RepositoryError::DataDirNotFound)?
            .join("ropy")
            .join("clipboard.db"))
    }

    fn needs_schema_migration(meta: &sled::Tree) -> Result<bool, RepositoryError> {
        match meta
            .get(b"schema_version")
            .map_err(|e| RepositoryError::Query(e.to_string()))?
        {
            Some(v) if v.len() == 8 => {
                let stored =
                    u64::from_be_bytes(v.as_ref().try_into().map_err(|_| {
                        RepositoryError::Deserialization("bad schema version".into())
                    })?);
                Ok(stored != SCHEMA_VERSION)
            }
            _ => Ok(true),
        }
    }
}

impl Drop for ClipboardRepository {
    fn drop(&mut self) {
        self.flush().ok();
    }
}

// ── Save operations ───────────────────────────────────────────────

impl ClipboardRepository {
    /// Save a clipboard record.
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

        if let Some(existing) = self.get_raw(&key)? {
            let mut record: ClipboardRecord = serde_json::from_slice(&existing)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            record.created_at = now;
            self.put_raw(&key, &record)?;
            self.time_index.upsert(&record)?;
            return Ok(record);
        }

        let record = ClipboardRecord {
            id,
            content,
            created_at: now,
            content_type,
            pinned: false,
        };
        self.put_raw(&key, &record)?;
        self.time_index.upsert(&record)?;
        Ok(record)
    }

    /// Save an image record from an existing file path.
    ///
    /// When a duplicate is found the newly saved image file is removed
    /// and only `created_at` is updated on the existing record.
    pub fn save_image_from_path(
        &self,
        file_path: String,
        image_content_hash: u64,
    ) -> Result<ClipboardRecord, RepositoryError> {
        let id = image_content_hash;
        let key = id.to_be_bytes();
        let now = Local::now();

        if let Some(existing) = self.get_raw(&key)? {
            let mut record: ClipboardRecord = serde_json::from_slice(&existing)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            if record.content != file_path {
                Self::remove_image_files(&file_path);
            }
            record.created_at = now;
            self.put_raw(&key, &record)?;
            self.time_index.upsert(&record)?;
            return Ok(record);
        }

        let record = ClipboardRecord {
            id,
            content: file_path,
            created_at: now,
            content_type: ContentType::Image,
            pinned: false,
        };
        self.put_raw(&key, &record)?;
        self.time_index.upsert(&record)?;
        Ok(record)
    }

    /// Save text content (convenience wrapper).
    pub fn save_text(&self, content: String) -> Result<ClipboardRecord, RepositoryError> {
        self.save(content, ContentType::Text)
    }
}

// ── Query operations ──────────────────────────────────────────────

impl ClipboardRepository {
    /// Get a record by ID.
    pub fn get_by_id(&self, id: u64) -> Result<Option<ClipboardRecord>, RepositoryError> {
        let key = id.to_be_bytes();
        match self.get_raw(&key)? {
            Some(value) => {
                let record = serde_json::from_slice(&value)
                    .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    /// Get the most recent `limit` records (pinned first).
    ///
    /// Uses the lightweight time index to select IDs, then batch-loads
    /// only the needed records from the main tree.
    pub fn get_recent(&self, limit: usize) -> Result<Vec<ClipboardRecord>, RepositoryError> {
        let selected_ids = self.time_index.select_recent_ids(limit)?;
        let mut records = self.load_records(&selected_ids);
        Self::sort_pinned_first(&mut records);
        Ok(records)
    }

    /// Search text records by keyword (case-insensitive).
    ///
    /// Uses the time index to skip non-text records.
    pub fn search(&self, keyword: &str) -> Result<Vec<ClipboardRecord>, RepositoryError> {
        let keyword_lower = keyword.to_lowercase();
        let text_ids = self.time_index.text_record_ids()?;

        let mut records = Vec::new();
        for id in text_ids {
            let key = id.to_be_bytes();
            if let Some(value) = self.get_raw(&key)? {
                let record: ClipboardRecord = match serde_json::from_slice(&value) {
                    Ok(r) => r,
                    Err(e) => {
                        tracing::warn!(error = %e, "skipping record that failed to deserialize");
                        continue;
                    }
                };
                if record.content.to_lowercase().contains(&keyword_lower) {
                    records.push(record);
                }
            }
        }
        Self::sort_pinned_first(&mut records);
        Ok(records)
    }

    /// Get the total number of records.
    pub fn count(&self) -> usize {
        self.records.len()
    }
}

// ── Mutation operations ───────────────────────────────────────────

impl ClipboardRepository {
    /// Toggle the pin state of a record.
    pub fn toggle_pin(&self, id: u64) -> Result<(), RepositoryError> {
        let mut record = self
            .get_by_id(id)?
            .ok_or_else(|| RepositoryError::Query("record not found".to_string()))?;

        record.pinned = !record.pinned;

        let key = id.to_be_bytes();
        self.put_raw(&key, &record)?;
        self.time_index.update_pinned(
            record.created_at.timestamp_millis(),
            record.id,
            record.pinned,
            &record.content_type,
        )?;
        Ok(())
    }

    /// Delete a single record.
    pub fn delete(&self, id: u64) -> Result<bool, RepositoryError> {
        let record = self.get_by_id(id)?;
        if let Some(ref rec) = record
            && rec.content_type == ContentType::Image
        {
            Self::remove_image_files(&rec.content);
        }

        let key = id.to_be_bytes();
        let removed = self
            .records
            .remove(key)
            .map_err(|e| RepositoryError::Delete(e.to_string()))?;

        if let Some(rec) = record {
            self.time_index
                .remove(rec.created_at.timestamp_millis(), rec.id)?;
        }
        Ok(removed.is_some())
    }

    /// Clear all records and images.
    pub fn clear(&self) -> Result<(), RepositoryError> {
        self.records
            .clear()
            .map_err(|e| RepositoryError::Delete(e.to_string()))?;
        self.time_index.clear()?;
        if self.images_dir.exists() {
            fs::remove_dir_all(&self.images_dir).ok();
        }
        Ok(())
    }

    /// Clean up old records, keeping the most recent `keep_count` records.
    ///
    /// Pinned records are never removed.
    pub fn cleanup_old_records(&self, keep_count: usize) -> Result<usize, RepositoryError> {
        let total = self.count();
        if total <= keep_count {
            return Ok(0);
        }

        let candidates = self.time_index.oldest_unpinned(total - keep_count)?;
        let mut removed = 0;

        for (ti_key, id) in candidates {
            let rec_key = id.to_be_bytes();
            // Delete associated image files if this is an image record
            if let Some(value) = self.get_raw(&rec_key)?
                && let Ok(record) = serde_json::from_slice::<ClipboardRecord>(&value)
                && record.content_type == ContentType::Image
            {
                Self::remove_image_files(&record.content);
            }
            self.records
                .remove(rec_key)
                .map_err(|e| RepositoryError::Delete(e.to_string()))?;
            self.time_index.remove_raw(&ti_key)?;
            removed += 1;
        }
        Ok(removed)
    }
}

// ── Internal helpers ──────────────────────────────────────────────

impl ClipboardRepository {
    /// Sort records with pinned items first, each group in descending time.
    pub(crate) fn sort_pinned_first(records: &mut [ClipboardRecord]) {
        records.sort_unstable_by(|a, b| match (a.pinned, b.pinned) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.created_at.cmp(&a.created_at),
        });
    }

    /// Raw sled get on the records tree.
    fn get_raw(&self, key: &[u8]) -> Result<Option<sled::IVec>, RepositoryError> {
        self.records
            .get(key)
            .map_err(|e| RepositoryError::Query(e.to_string()))
    }

    /// Serialize and insert a record into the records tree.
    fn put_raw(&self, key: &[u8], record: &ClipboardRecord) -> Result<(), RepositoryError> {
        let value = serde_json::to_vec(record)
            .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
        self.records
            .insert(key, value)
            .map_err(|e| RepositoryError::Insert(e.to_string()))?;
        Ok(())
    }

    /// Load multiple records by ID, silently skipping failures.
    fn load_records(&self, ids: &[u64]) -> Vec<ClipboardRecord> {
        let mut out = Vec::with_capacity(ids.len());
        for &id in ids {
            let key = id.to_be_bytes();
            if let Ok(Some(value)) = self.get_raw(&key) {
                match serde_json::from_slice::<ClipboardRecord>(&value) {
                    Ok(record) => out.push(record),
                    Err(e) => {
                        tracing::warn!(error = %e, "skipping record that failed to deserialize");
                    }
                }
            }
        }
        out
    }

    /// Remove image file and its thumbnail.
    fn remove_image_files(path: &str) {
        let _ = fs::remove_file(path);
        let thumb_path = path.replace(".png", "_thumb.png");
        let _ = fs::remove_file(thumb_path);
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
        // Simulate a record stored with old `category` field and a
        // corresponding time_index entry (as would exist after schema v3).
        let repo = create_test_repo();
        let now = chrono::Local::now();
        let old_json = serde_json::json!({
            "id": 1000_u64,
            "content": "legacy record",
            "created_at": now,
            "content_type": "Text",
            "category": "Pinned"
        });
        let key = 1000_u64.to_be_bytes();
        let value = serde_json::to_vec(&old_json).expect("failed to serialize");
        repo.records.insert(key, value).expect("failed to insert");

        // Insert matching time_index entry
        repo.time_index
            .insert_raw(now.timestamp_millis(), 1000, false, &ContentType::Text);

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

        // Insert a valid record (via save_text, which also inserts time_index)
        repo.save_text("valid hello".to_string())
            .expect("Failed to save");

        // Insert corrupt data into records tree and a matching time_index entry
        let corrupt_id = 9999_u64;
        let corrupt_key = corrupt_id.to_be_bytes();
        repo.records
            .insert(corrupt_key, b"not valid json")
            .expect("failed to insert corrupt");
        repo.time_index
            .insert_raw(0, corrupt_id, false, &ContentType::Text);

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

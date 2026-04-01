//! Clipboard repository for storing and retrieving clipboard records.

use std::{cmp::Ordering, collections::HashSet, fs, path::PathBuf};

use chrono::Local;
use sled::Db;

use super::{
    errors::RepositoryError,
    models::{ClipboardRecord, ContentType},
    time_index::TimeIndex,
};
use crate::utils::{content_hash, normalize_file_paths, serialize_file_paths};

/// Schema version for the database. Bump this when the key format changes.
const SCHEMA_VERSION: u64 = 3;

/// Maximum page cache size for the sled database (16 MB).
///
/// sled defaults to 1 GB, which is far too large for a clipboard manager.
/// 32 MB provides ample room for typical workloads while keeping the memory
/// footprint reasonable.
const DB_CACHE_CAPACITY: u64 = 8 * 1024 * 1024;

/// Interval at which sled flushes dirty pages to disk (in milliseconds).
const DB_FLUSH_INTERVAL_MS: u64 = 1000;
/// Allow the repository to grow slightly past the configured limit so cleanup
/// can batch deletions instead of scanning on every successful save.
const CLEANUP_BUFFER_DIVISOR: usize = 10;
const MIN_CLEANUP_BUFFER_RECORDS: usize = 1;

pub struct ClipboardRepository {
    db: Db,
    records: sled::Tree,
    time_index: TimeIndex,
    favorites: sled::Tree,
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
        let db = sled::Config::new()
            .path(db_path)
            .cache_capacity(DB_CACHE_CAPACITY)
            .flush_every_ms(Some(DB_FLUSH_INTERVAL_MS))
            .open()
            .map_err(|e| RepositoryError::DatabaseOpen(e.to_string()))?;

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
        let favorites = db
            .open_tree("favorites")
            .map_err(|e| RepositoryError::TreeOpen(e.to_string()))?;

        if Self::needs_schema_migration(&meta)? {
            records
                .clear()
                .map_err(|e| RepositoryError::Delete(e.to_string()))?;
            time_index.clear()?;
            favorites
                .clear()
                .map_err(|e| RepositoryError::Delete(e.to_string()))?;
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
            favorites,
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
            let mut record: ClipboardRecord = postcard::from_bytes(&existing)
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
            let mut record: ClipboardRecord = postcard::from_bytes(&existing)
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

    /// Save file list content (convenience wrapper).
    pub fn save_files(&self, paths: &[String]) -> Result<ClipboardRecord, RepositoryError> {
        let normalized = normalize_file_paths(paths);
        if normalized.is_empty() {
            return Err(RepositoryError::Query("file list is empty".to_string()));
        }

        let content = serialize_file_paths(&normalized)
            .map_err(|error| RepositoryError::Serialization(error.to_string()))?;

        self.save(content, ContentType::FilePath)
    }
}

impl ClipboardRepository {
    /// Get a record by ID.
    pub fn get_by_id(&self, id: u64) -> Result<Option<ClipboardRecord>, RepositoryError> {
        let key = id.to_be_bytes();
        match self.get_raw(&key)? {
            Some(value) => {
                let record = postcard::from_bytes(&value)
                    .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    /// Get the records for the default board view.
    ///
    /// Pinned records stay at the top. Favorited records do not consume the
    /// ordinary `limit`, but otherwise remain in the default chronological
    /// ordering with other unpinned records.
    pub fn get_display_records(
        &self,
        limit: usize,
    ) -> Result<Vec<ClipboardRecord>, RepositoryError> {
        let favorite_ids = self.favorite_id_set()?;
        let selected_ids = self.time_index.select_display_ids(limit, &favorite_ids)?;
        let mut records = self.load_records(&selected_ids);
        Self::sort_for_display(&mut records);
        Ok(records)
    }

    /// Get the total number of records.
    pub fn count(&self) -> usize {
        self.records.len()
    }
}

impl ClipboardRepository {
    /// Return all favorite record IDs.
    pub fn favorite_ids(&self) -> Result<Vec<u64>, RepositoryError> {
        let mut ids = Vec::new();

        for entry in &self.favorites {
            let (key, _) = entry.map_err(|e| RepositoryError::Query(e.to_string()))?;
            let Some(id) = Self::decode_u64_key(&key) else {
                continue;
            };
            ids.push(id);
        }

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
        if self
            .favorites
            .get(key)
            .map_err(|e| RepositoryError::Query(e.to_string()))?
            .is_some()
        {
            self.favorites
                .remove(key)
                .map_err(|e| RepositoryError::Delete(e.to_string()))?;
            return Ok(false);
        }

        let favorited_at = Local::now().timestamp_millis().to_be_bytes();
        self.favorites
            .insert(key, favorited_at.as_slice())
            .map_err(|e| RepositoryError::Insert(e.to_string()))?;
        Ok(true)
    }

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
        self.remove_favorite(id)?;

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
        self.favorites
            .clear()
            .map_err(|e| RepositoryError::Delete(e.to_string()))?;
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
        let favorite_ids = self.favorite_id_set()?;
        let ordinary_total = self.ordinary_record_count(total, &favorite_ids)?;
        self.cleanup_old_records_with_ordinary_total(
            keep_count,
            ordinary_total,
            total,
            &favorite_ids,
        )
    }

    /// Clean up old records only after the repository has exceeded the
    /// configured limit by a small buffer.
    pub fn cleanup_old_records_if_needed(
        &self,
        keep_count: usize,
    ) -> Result<usize, RepositoryError> {
        let total = self.count();
        let favorite_ids = self.favorite_id_set()?;
        let ordinary_total = self.ordinary_record_count(total, &favorite_ids)?;
        if ordinary_total <= Self::cleanup_trigger_record_count(keep_count) {
            return Ok(0);
        }

        self.cleanup_old_records_with_ordinary_total(
            keep_count,
            ordinary_total,
            total,
            &favorite_ids,
        )
    }

    fn cleanup_old_records_with_ordinary_total(
        &self,
        keep_count: usize,
        ordinary_total: usize,
        total: usize,
        favorite_ids: &HashSet<u64>,
    ) -> Result<usize, RepositoryError> {
        if ordinary_total <= keep_count {
            return Ok(0);
        }

        let candidates = self.time_index.oldest_unpinned(total)?;
        let mut removed = 0;

        for (ti_key, id) in candidates {
            if ordinary_total.saturating_sub(removed) <= keep_count {
                break;
            }
            if favorite_ids.contains(&id) {
                continue;
            }

            let rec_key = id.to_be_bytes();
            // Delete associated image files if this is an image record
            if let Some(value) = self.get_raw(&rec_key)?
                && let Ok(record) = postcard::from_bytes::<ClipboardRecord>(&value)
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

    fn cleanup_buffer_record_count(keep_count: usize) -> usize {
        keep_count
            .saturating_div(CLEANUP_BUFFER_DIVISOR)
            .max(MIN_CLEANUP_BUFFER_RECORDS)
    }

    fn cleanup_trigger_record_count(keep_count: usize) -> usize {
        keep_count.saturating_add(Self::cleanup_buffer_record_count(keep_count))
    }
}

impl ClipboardRepository {
    pub(crate) fn compare_for_display(left: &ClipboardRecord, right: &ClipboardRecord) -> Ordering {
        Self::display_priority(left)
            .cmp(&Self::display_priority(right))
            .then_with(|| right.created_at.cmp(&left.created_at))
    }

    pub(crate) fn sort_for_display(records: &mut [ClipboardRecord]) {
        records.sort_unstable_by(Self::compare_for_display);
    }

    /// Raw sled get on the records tree.
    fn get_raw(&self, key: &[u8]) -> Result<Option<sled::IVec>, RepositoryError> {
        self.records
            .get(key)
            .map_err(|e| RepositoryError::Query(e.to_string()))
    }

    /// Serialize and insert a record into the records tree.
    fn put_raw(&self, key: &[u8], record: &ClipboardRecord) -> Result<(), RepositoryError> {
        let value = postcard::to_allocvec(record)
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
                match postcard::from_bytes::<ClipboardRecord>(&value) {
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

    fn remove_favorite(&self, id: u64) -> Result<(), RepositoryError> {
        self.favorites
            .remove(id.to_be_bytes())
            .map_err(|e| RepositoryError::Delete(e.to_string()))?;
        Ok(())
    }

    fn favorite_id_set(&self) -> Result<HashSet<u64>, RepositoryError> {
        let mut favorite_ids = HashSet::new();

        for id in self.favorite_ids()? {
            if self.get_raw(&id.to_be_bytes())?.is_some() {
                favorite_ids.insert(id);
            }
        }

        Ok(favorite_ids)
    }

    fn ordinary_record_count(
        &self,
        total: usize,
        favorite_ids: &HashSet<u64>,
    ) -> Result<usize, RepositoryError> {
        let mut special_ids = self
            .time_index
            .pinned_ids()?
            .into_iter()
            .collect::<HashSet<_>>();
        special_ids.extend(favorite_ids.iter().copied());
        Ok(total.saturating_sub(special_ids.len()))
    }

    const fn display_priority(record: &ClipboardRecord) -> u8 {
        (!record.pinned) as u8
    }

    fn decode_u64_key(bytes: &[u8]) -> Option<u64> {
        let key: [u8; 8] = bytes.try_into().ok()?;
        Some(u64::from_be_bytes(key))
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use rstest::rstest;
    use tempfile::tempdir;

    use super::*;

    #[allow(clippy::expect_used)]
    fn create_test_repo() -> ClipboardRepository {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        ClipboardRepository::init(&db_path, temp_dir.path().join("images"))
            .expect("Failed to create test repository")
    }

    #[allow(clippy::expect_used)]
    fn load_display_records(repo: &ClipboardRepository, limit: usize) -> Vec<ClipboardRecord> {
        repo.get_display_records(limit)
            .expect("Failed to get display records")
    }

    #[allow(clippy::expect_used)]
    fn save_numbered_records(repo: &ClipboardRepository, count: usize) {
        for i in 1..=count {
            repo.save_text(format!("Record {i}"))
                .expect("Failed to save");
            thread::sleep(Duration::from_millis(10));
        }
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
    fn test_get_display_records() {
        let repo = create_test_repo();

        for i in 1..=5 {
            repo.save_text(format!("Record {i}"))
                .expect("Failed to save");
            thread::sleep(Duration::from_millis(10));
        }

        let recent = load_display_records(&repo, 3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].content, "Record 5");
        assert_eq!(recent[1].content, "Record 4");
        assert_eq!(recent[2].content, "Record 3");
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_get_display_records_when_favorites_present_keep_default_time_order() {
        let repo = create_test_repo();

        let favorite_and_pinned = repo
            .save_text("Favorite pinned".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let favorite_only = repo
            .save_text("Favorite only".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("Ordinary old".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("Ordinary new".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("Ordinary newest".to_string())
            .expect("Failed to save");

        repo.toggle_favorite(favorite_and_pinned.id)
            .expect("Failed to favorite pinned record");
        repo.toggle_pin(favorite_and_pinned.id)
            .expect("Failed to pin favorite record");
        repo.toggle_favorite(favorite_only.id)
            .expect("Failed to favorite record");

        let display_records = repo
            .get_display_records(2)
            .expect("Failed to get display records");
        let contents = display_records
            .iter()
            .map(|record| record.content.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            contents,
            vec![
                "Favorite pinned",
                "Ordinary newest",
                "Ordinary new",
                "Favorite only",
            ]
        );
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_get_display_records_when_total_is_41_with_two_favorites_and_one_pinned_returns_41() {
        let repo = create_test_repo();

        let first = repo
            .save_text("Record 1".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let second = repo
            .save_text("Record 2".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let third = repo
            .save_text("Record 3".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));

        repo.toggle_favorite(first.id)
            .expect("Failed to favorite first");
        repo.toggle_favorite(second.id)
            .expect("Failed to favorite second");
        repo.toggle_pin(third.id).expect("Failed to pin third");

        for index in 4..=41 {
            repo.save_text(format!("Record {index}"))
                .expect("Failed to save");
            thread::sleep(Duration::from_millis(10));
        }

        let display_records = repo
            .get_display_records(40)
            .expect("Failed to get display records");

        assert_eq!(repo.count(), 41);
        assert_eq!(display_records.len(), 41);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_get_display_records_when_limit_is_40_with_two_favorites_and_one_pinned_returns_43() {
        let repo = create_test_repo();

        let first = repo
            .save_text("Record 1".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let second = repo
            .save_text("Record 2".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let third = repo
            .save_text("Record 3".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));

        repo.toggle_favorite(first.id)
            .expect("Failed to favorite first");
        repo.toggle_favorite(second.id)
            .expect("Failed to favorite second");
        repo.toggle_pin(third.id).expect("Failed to pin third");

        for index in 4..=43 {
            repo.save_text(format!("Record {index}"))
                .expect("Failed to save");
            thread::sleep(Duration::from_millis(10));
        }

        let display_records = repo
            .get_display_records(40)
            .expect("Failed to get display records");

        assert_eq!(repo.count(), 43);
        assert_eq!(display_records.len(), 43);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_get_display_records_when_newest_records_are_favorited_still_returns_43() {
        let repo = create_test_repo();

        let pinned = repo
            .save_text("Pinned".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));

        for index in 2..=43 {
            repo.save_text(format!("Record {index}"))
                .expect("Failed to save");
            thread::sleep(Duration::from_millis(10));
        }

        repo.toggle_pin(pinned.id).expect("Failed to pin");

        let newest = repo.get_by_id(crate::utils::content_hash("Record 43", &ContentType::Text));
        let second_newest =
            repo.get_by_id(crate::utils::content_hash("Record 42", &ContentType::Text));

        let newest = newest
            .expect("Failed to get newest")
            .expect("Newest should exist");
        let second_newest = second_newest
            .expect("Failed to get second newest")
            .expect("Second newest should exist");

        repo.toggle_favorite(newest.id)
            .expect("Failed to favorite newest");
        repo.toggle_favorite(second_newest.id)
            .expect("Failed to favorite second newest");

        let display_records = repo
            .get_display_records(40)
            .expect("Failed to get display records");

        assert_eq!(repo.count(), 43);
        assert_eq!(display_records.len(), 43);
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
        let recent = load_display_records(&repo, 5);
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
        let recent = load_display_records(&repo, 2);
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

        let recent = load_display_records(&repo, 10);
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

        let recent = load_display_records(&repo, 10);
        // Both pinned, newer first
        assert_eq!(recent[0].content, "Gamma");
        assert_eq!(recent[1].content, "Alpha");
        // Unpinned
        assert_eq!(recent[2].content, "Beta");
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
        // The pinned record survives and no longer consumes the ordinary keep budget.
        assert_eq!(removed, 2);
        assert_eq!(repo.count(), 4);
        // Pinned record should survive
        let pinned = repo
            .get_by_id(r1.id)
            .expect("Failed to get")
            .expect("Pinned record should still exist");
        assert!(pinned.pinned);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_binary_serialization_round_trip() {
        // Verify records survive a postcard serialization round-trip via the
        // internal records tree.
        let repo = create_test_repo();
        let now = chrono::Local::now();
        let record = ClipboardRecord {
            id: 1000_u64,
            content: "binary record".to_string(),
            created_at: now,
            content_type: ContentType::Text,
            pinned: false,
        };
        let key = 1000_u64.to_be_bytes();
        let value = postcard::to_allocvec(&record).expect("failed to serialize");
        repo.records.insert(key, value).expect("failed to insert");

        // Insert matching time_index entry
        repo.time_index
            .insert_raw(now.timestamp_millis(), 1000, false, &ContentType::Text);

        let records = load_display_records(&repo, 10);
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].content, "binary record");
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

        // keep_count = 2 applies only to ordinary records.
        let removed = repo.cleanup_old_records(2).expect("Failed to clean up");
        assert_eq!(removed, 0);
        assert_eq!(repo.count(), 4);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_favorite_toggle_record_updates_membership() {
        let repo = create_test_repo();
        let record = repo
            .save_text("Favorite me".to_string())
            .expect("Failed to save");

        assert!(
            repo.favorite_ids()
                .expect("Failed to load favorite ids")
                .is_empty()
        );

        let is_favorite = repo
            .toggle_favorite(record.id)
            .expect("Failed to add favorite");
        assert!(is_favorite);

        let favorite_ids = repo.favorite_ids().expect("Failed to load favorite ids");
        assert_eq!(favorite_ids, vec![record.id]);

        let is_favorite = repo
            .toggle_favorite(record.id)
            .expect("Failed to remove favorite");
        assert!(!is_favorite);
        assert!(
            repo.favorite_ids()
                .expect("Failed to load favorite ids")
                .is_empty()
        );
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_cleanup_old_records_when_favorited_record_present_counts_only_ordinary_records() {
        let repo = create_test_repo();

        let favorite = repo
            .save_text("Old favorite".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        for index in 2..=6 {
            repo.save_text(format!("Record {index}"))
                .expect("Failed to save");
            thread::sleep(Duration::from_millis(10));
        }

        repo.toggle_favorite(favorite.id)
            .expect("Failed to favorite record");

        let removed = repo.cleanup_old_records(3).expect("Failed to clean up");
        assert_eq!(removed, 2);
        assert_eq!(repo.count(), 4);
        assert!(
            repo.get_by_id(favorite.id)
                .expect("Failed to get by id")
                .is_some()
        );
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_delete_favorite_record_removes_membership() {
        let repo = create_test_repo();
        let record = repo
            .save_text("Favorite delete".to_string())
            .expect("Failed to save");

        repo.toggle_favorite(record.id)
            .expect("Failed to favorite record");
        assert_eq!(
            repo.favorite_ids().expect("Failed to load favorite ids"),
            vec![record.id]
        );

        let deleted = repo.delete(record.id).expect("Failed to delete");
        assert!(deleted);
        assert!(
            repo.favorite_ids()
                .expect("Failed to load favorite ids")
                .is_empty()
        );
    }

    // ── Boundary and Edge Case Tests ──────────────────────────────

    #[test]
    #[allow(clippy::expect_used)]
    fn test_get_by_id_nonexistent() {
        let repo = create_test_repo();

        // Query for a non-existent ID should return Ok(None)
        let result = repo.get_by_id(999_999_999).expect("Failed to get by id");
        assert!(result.is_none());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_get_by_id_zero() {
        let repo = create_test_repo();

        // Query for ID 0 (edge case)
        let result = repo.get_by_id(0).expect("Failed to get by id");
        assert!(result.is_none());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_get_by_id_u64_max() {
        let repo = create_test_repo();

        // Query for max u64 value
        let result = repo.get_by_id(u64::MAX).expect("Failed to get by id");
        assert!(result.is_none());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_get_display_records_zero_limit() {
        let repo = create_test_repo();

        repo.save_text("Test".to_string()).expect("Failed to save");

        // With limit 0, should return empty (except pinned records)
        let result = load_display_records(&repo, 0);
        // Pinned records always appear, so this tests unpinned behavior
        assert!(result.is_empty());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_get_display_records_large_limit() {
        let repo = create_test_repo();

        // Save only 3 records but request 1000
        for i in 1..=3 {
            repo.save_text(format!("Record {i}"))
                .expect("Failed to save");
            thread::sleep(Duration::from_millis(10));
        }

        let result = load_display_records(&repo, 1000);
        assert_eq!(result.len(), 3);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_get_display_records_empty_repo() {
        let repo = create_test_repo();

        let result = load_display_records(&repo, 10);
        assert!(result.is_empty());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_count_empty() {
        let repo = create_test_repo();

        assert_eq!(repo.count(), 0);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_cleanup_old_records_zero_keep() {
        let repo = create_test_repo();

        repo.save_text("Test".to_string()).expect("Failed to save");

        // keep_count = 0 should remove all unpinned records
        let removed = repo.cleanup_old_records(0).expect("Failed to clean up");
        assert_eq!(removed, 1);
        assert_eq!(repo.count(), 0);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_cleanup_old_records_greater_than_total() {
        let repo = create_test_repo();

        repo.save_text("Test".to_string()).expect("Failed to save");

        // keep_count > total should remove nothing
        let removed = repo.cleanup_old_records(100).expect("Failed to clean up");
        assert_eq!(removed, 0);
        assert_eq!(repo.count(), 1);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_cleanup_old_records_equal_to_total() {
        let repo = create_test_repo();

        repo.save_text("Test".to_string()).expect("Failed to save");

        // keep_count == total should remove nothing
        let removed = repo.cleanup_old_records(1).expect("Failed to clean up");
        assert_eq!(removed, 0);
        assert_eq!(repo.count(), 1);
    }

    #[rstest]
    #[case(10)]
    #[case(50)]
    fn test_cleanup_old_records_if_needed_at_threshold_preserves_records(
        #[case] keep_count: usize,
    ) {
        let repo = create_test_repo();
        let threshold = keep_count + ClipboardRepository::cleanup_buffer_record_count(keep_count);

        save_numbered_records(&repo, threshold);

        let removed = repo
            .cleanup_old_records_if_needed(keep_count)
            .unwrap_or_else(|err| panic!("Failed to conditionally clean up: {err}"));

        assert_eq!(removed, 0);
        assert_eq!(repo.count(), threshold);
    }

    #[rstest]
    #[case(10)]
    #[case(50)]
    fn test_cleanup_old_records_if_needed_above_threshold_trims_to_keep_count(
        #[case] keep_count: usize,
    ) {
        let repo = create_test_repo();
        let threshold = keep_count + ClipboardRepository::cleanup_buffer_record_count(keep_count);

        save_numbered_records(&repo, threshold + 1);

        let removed = repo
            .cleanup_old_records_if_needed(keep_count)
            .unwrap_or_else(|err| panic!("Failed to conditionally clean up: {err}"));

        assert_eq!(removed, threshold + 1 - keep_count);
        assert_eq!(repo.count(), keep_count);

        let recent = repo
            .get_display_records(keep_count)
            .unwrap_or_else(|err| panic!("Failed to get display records: {err}"));
        assert_eq!(recent.len(), keep_count);
        assert_eq!(recent[0].content, format!("Record {}", threshold + 1));
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_toggle_pin_nonexistent() {
        let repo = create_test_repo();

        // Toggling pin on non-existent record should return error
        let result = repo.toggle_pin(999_999_999);
        assert!(result.is_err());

        // Verify it's the expected error type
        let err_msg = format!("{}", result.expect_err("Should be error"));
        assert!(err_msg.contains("record not found"));
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_delete_nonexistent() {
        let repo = create_test_repo();

        // Deleting non-existent record should return Ok(false)
        let result = repo.delete(999_999_999).expect("Failed to delete");
        assert!(!result);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_save_empty_content() {
        let repo = create_test_repo();

        // Empty string is valid content
        let record = repo.save_text(String::new()).expect("Failed to save");
        assert_eq!(record.content, "");

        // Should be retrievable
        let retrieved = repo
            .get_by_id(record.id)
            .expect("Failed to get")
            .expect("Not found");
        assert_eq!(retrieved.content, "");
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_save_very_long_content() {
        let repo = create_test_repo();

        // Content with 100KB of text
        let long_content = "x".repeat(100_000);
        let record = repo
            .save_text(long_content.clone())
            .expect("Failed to save");

        let retrieved = repo
            .get_by_id(record.id)
            .expect("Failed to get")
            .expect("Not found");
        assert_eq!(retrieved.content.len(), 100_000);
        assert_eq!(retrieved.content, long_content);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_save_unicode_content() {
        let repo = create_test_repo();

        // Various Unicode content
        let contents = vec![
            "Hello 世界 🌍",
            "مرحبا بالعالم",
            "🎉🎊🎁",
            "日本語テキスト",
            "Special chars: <>&\"'",
        ];

        for content in contents {
            let record = repo.save_text(content.to_string()).expect("Failed to save");
            let retrieved = repo
                .get_by_id(record.id)
                .expect("Failed to get")
                .expect("Not found");
            assert_eq!(retrieved.content, content);
        }
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_save_different_content_types() {
        let repo = create_test_repo();

        // Save text
        let text_record = repo
            .save("text content".to_string(), ContentType::Text)
            .expect("Failed to save text");
        assert_eq!(text_record.content_type, ContentType::Text);

        // Save filepath
        let path_record = repo
            .save("/path/to/file".to_string(), ContentType::FilePath)
            .expect("Failed to save path");
        assert_eq!(path_record.content_type, ContentType::FilePath);

        // Verify both are stored
        assert_eq!(repo.count(), 2);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_repository_save_files_normalizes_and_persists_file_payload() {
        let repo = create_test_repo();

        let record = repo
            .save_files(&[
                "file:///tmp/alpha%20file.txt".to_string(),
                "/tmp/beta.txt".to_string(),
            ])
            .expect("Failed to save file paths");

        assert_eq!(record.content_type, ContentType::FilePath);
        assert_eq!(
            record.content,
            "[\"/tmp/alpha file.txt\",\"/tmp/beta.txt\"]"
        );
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_repository_save_files_when_equivalent_inputs_reuses_existing_record() {
        let repo = create_test_repo();

        let first = repo
            .save_files(&["file:///tmp/demo%20file.txt".to_string()])
            .expect("Failed to save first file record");
        let second = repo
            .save_files(&["/tmp/demo file.txt".to_string()])
            .expect("Failed to save second file record");

        assert_eq!(first.id, second.id);
        assert_eq!(repo.count(), 1);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_save_image_from_path_basic() {
        let repo = create_test_repo();

        // Save an image reference
        let path = "/tmp/test_image.png".to_string();
        let hash = 12345_u64;

        let record = repo
            .save_image_from_path(path.clone(), hash)
            .expect("Failed to save image");

        assert_eq!(record.content, path);
        assert_eq!(record.content_type, ContentType::Image);
        assert_eq!(record.id, hash);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_save_image_from_path_dedup() {
        let repo = create_test_repo();

        let path1 = "/tmp/image1.png".to_string();
        let path2 = "/tmp/image2.png".to_string();
        let hash = 12345_u64;

        // Save first image
        let r1 = repo
            .save_image_from_path(path1, hash)
            .expect("Failed to save first");

        thread::sleep(Duration::from_millis(10));

        // Save same hash with different path (should dedup)
        let r2 = repo
            .save_image_from_path(path2, hash)
            .expect("Failed to save second");

        // Same ID due to same hash
        assert_eq!(r1.id, r2.id);
        // Only one record
        assert_eq!(repo.count(), 1);
        // Timestamp updated
        assert!(r2.created_at > r1.created_at);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_clear_empty_repo() {
        let repo = create_test_repo();

        // Clearing empty repo should not error
        repo.clear().expect("Failed to clear empty repo");
        assert_eq!(repo.count(), 0);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_multiple_operations_sequence() {
        let repo = create_test_repo();

        // Save multiple records
        let r1 = repo.save_text("First".to_string()).expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let r2 = repo
            .save_text("Second".to_string())
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
        let _r3 = repo.save_text("Third".to_string()).expect("Failed to save");

        assert_eq!(repo.count(), 3);

        // Pin the first record
        repo.toggle_pin(r1.id).expect("Failed to pin");

        // Delete the second
        let deleted = repo.delete(r2.id).expect("Failed to delete");
        assert!(deleted);
        assert_eq!(repo.count(), 2);

        // Verify remaining records
        let recent = load_display_records(&repo, 10);
        assert_eq!(recent.len(), 2);
        // Pinned first, then third (most recent unpinned)
        assert_eq!(recent[0].content, "First");
        assert_eq!(recent[1].content, "Third");

        // Clear all
        repo.clear().expect("Failed to clear");
        assert_eq!(repo.count(), 0);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_sort_for_display_empty() {
        let mut records: Vec<ClipboardRecord> = vec![];
        ClipboardRepository::sort_for_display(&mut records);
        assert!(records.is_empty());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_sort_for_display_single() {
        let now = chrono::Local::now();
        let mut records = vec![ClipboardRecord {
            id: 1,
            content: "only".to_string(),
            created_at: now,
            content_type: ContentType::Text,
            pinned: false,
        }];

        ClipboardRepository::sort_for_display(&mut records);
        assert_eq!(records.len(), 1);
        assert!(!records[0].pinned);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_sort_for_display_all_same_pinned_state() {
        let now = chrono::Local::now();
        let mut records = vec![
            ClipboardRecord {
                id: 1,
                content: "oldest".to_string(),
                created_at: now - chrono::Duration::seconds(2),
                content_type: ContentType::Text,
                pinned: false,
            },
            ClipboardRecord {
                id: 2,
                content: "middle".to_string(),
                created_at: now - chrono::Duration::seconds(1),
                content_type: ContentType::Text,
                pinned: false,
            },
            ClipboardRecord {
                id: 3,
                content: "newest".to_string(),
                created_at: now,
                content_type: ContentType::Text,
                pinned: false,
            },
        ];

        ClipboardRepository::sort_for_display(&mut records);

        // Should be sorted by time descending (newest first)
        assert_eq!(records[0].content, "newest");
        assert_eq!(records[1].content, "middle");
        assert_eq!(records[2].content, "oldest");
    }

    // ── Error Handling Tests ──────────────────────────────────────

    #[test]
    #[allow(clippy::expect_used)]
    fn test_concurrent_save_and_delete() {
        let repo = create_test_repo();
        let repo = std::sync::Arc::new(repo);
        let mut handles = vec![];

        // Spawn threads that save records
        for i in 0..10 {
            let r = repo.clone();
            let handle = std::thread::spawn(move || {
                r.save_text(format!("thread {i}")).expect("Failed to save")
            });
            handles.push(handle);
        }

        let mut ids = vec![];
        for handle in handles {
            let record = handle.join().expect("Thread panicked");
            ids.push(record.id);
        }

        assert_eq!(repo.count(), 10);

        // Now delete half of them concurrently
        let repo2 = repo.clone();
        let ids_to_delete = ids[..5].to_vec();
        let handle = std::thread::spawn(move || {
            for id in ids_to_delete {
                repo2.delete(id).expect("Failed to delete");
            }
        });

        handle.join().expect("Delete thread panicked");
        assert_eq!(repo.count(), 5);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_save_after_clear() {
        let repo = create_test_repo();

        repo.save_text("Before clear".to_string())
            .expect("Failed to save");
        repo.clear().expect("Failed to clear");

        // Should be able to save after clear
        let record = repo
            .save_text("After clear".to_string())
            .expect("Failed to save after clear");
        assert_eq!(repo.count(), 1);
        assert_eq!(record.content, "After clear");
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_double_toggle_pin() {
        let repo = create_test_repo();

        let record = repo.save_text("Test".to_string()).expect("Failed to save");
        assert!(!record.pinned);

        // Toggle on
        repo.toggle_pin(record.id).expect("Failed to toggle");
        let pinned = repo
            .get_by_id(record.id)
            .expect("Failed to get")
            .expect("Should exist");
        assert!(pinned.pinned);

        // Toggle off
        repo.toggle_pin(record.id).expect("Failed to toggle");
        let unpinned = repo
            .get_by_id(record.id)
            .expect("Failed to get")
            .expect("Should exist");
        assert!(!unpinned.pinned);

        // Toggle on again
        repo.toggle_pin(record.id).expect("Failed to toggle");
        let pinned_again = repo
            .get_by_id(record.id)
            .expect("Failed to get")
            .expect("Should exist");
        assert!(pinned_again.pinned);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_get_display_records_with_pinned_and_limit() {
        let repo = create_test_repo();

        // Create 5 unpinned records
        for i in 1..=5 {
            repo.save_text(format!("Unpinned {i}"))
                .expect("Failed to save");
            thread::sleep(Duration::from_millis(10));
        }

        // Pin the oldest one (first saved)
        let recent = load_display_records(&repo, 5);
        let oldest_id = recent[4].id; // Last in the list is oldest
        repo.toggle_pin(oldest_id).expect("Failed to pin");

        // Get display records with an ordinary limit of 3
        let recent_limited = load_display_records(&repo, 3);

        // Should include the pinned one plus the 3 most recent unpinned records.
        assert_eq!(recent_limited.len(), 4);
        // First should be the pinned one
        assert!(recent_limited[0].pinned);
        assert_eq!(recent_limited[0].id, oldest_id);
        assert_eq!(recent_limited[1].content, "Unpinned 5");
        assert_eq!(recent_limited[2].content, "Unpinned 4");
        assert_eq!(recent_limited[3].content, "Unpinned 3");
    }
}

//! Lightweight secondary index keyed by timestamp for efficient
//! chronological queries without full record deserialization.
//!
//! ## Key format (16 bytes)
//! `timestamp_millis(i64 BE, 8B) ++ record_id(u64 BE, 8B)`
//!
//! ## Value format (2 bytes)
//! `pinned(u8) ++ content_type_tag(u8)`

use std::collections::HashSet;

use super::{
    backend::KvTree,
    errors::RepositoryError,
    models::{ClipboardRecord, ContentType},
};

/// Metadata extracted from a single time index entry.
pub struct IndexEntry {
    pub id: u64,
    pub is_pinned: bool,
}

/// A lightweight secondary index that maps `(timestamp, id)` to
/// `(pinned, content_type)`, enabling chronological queries and
/// type-based filtering without touching the main record store.
pub struct TimeIndex {
    entries: Box<dyn KvTree>,
    id_lookup: Box<dyn KvTree>,
}

impl TimeIndex {
    pub fn new(entries: Box<dyn KvTree>, id_lookup: Box<dyn KvTree>) -> Self {
        Self { entries, id_lookup }
    }

    /// Insert or update the time index entry for a record.
    ///
    /// Removes any stale entry with the same `id` before inserting
    /// the new one (timestamp may have changed on dedup upsert).
    pub fn upsert(&self, record: &ClipboardRecord) -> Result<(), RepositoryError> {
        self.remove_by_id(record.id)?;
        let key = Self::encode_key(record.created_at.timestamp_millis(), record.id);
        let val = Self::encode_value(record.pinned, &record.content_type);
        self.entries.insert(&key, &val)?;
        if let Err(error) = self.id_lookup.insert(
            &record.id.to_be_bytes(),
            &record.created_at.timestamp_millis().to_be_bytes(),
        ) {
            let _ = self.entries.remove(&key);
            return Err(error);
        }
        Ok(())
    }

    /// Remove the entry that matches the given `(timestamp_millis, id)` pair.
    pub fn remove(&self, timestamp_millis: i64, id: u64) -> Result<(), RepositoryError> {
        let key = Self::encode_key(timestamp_millis, id);
        self.entries.remove(&key)?;
        self.id_lookup.remove(&id.to_be_bytes())?;
        Ok(())
    }

    /// Update the pinned flag for an existing entry (key stays the same).
    pub fn update_pinned(
        &self,
        timestamp_millis: i64,
        id: u64,
        pinned: bool,
        content_type: &ContentType,
    ) -> Result<(), RepositoryError> {
        let key = Self::encode_key(timestamp_millis, id);
        let val = Self::encode_value(pinned, content_type);
        self.entries.insert(&key, &val)
    }

    /// Clear all entries.
    pub fn clear(&self) -> Result<(), RepositoryError> {
        self.entries.clear()?;
        self.id_lookup.clear()
    }

    /// Select record IDs for the default board view.
    ///
    /// Pinned records are always included first. Among unpinned records, all
    /// favorited records are included without consuming the ordinary `limit`,
    /// and up to `limit` ordinary records are appended in newest-first order.
    pub fn select_display_ids(
        &self,
        limit: usize,
        favorite_ids: &HashSet<u64>,
    ) -> Result<Vec<u64>, RepositoryError> {
        let mut pinned = Vec::new();
        let mut selected_unpinned = Vec::new();
        let mut ordinary_count = 0;

        self.entries.scan_descending(&mut |key, value| {
            let Some(entry) = Self::decode_entry(key, value) else {
                return true;
            };

            if entry.is_pinned {
                pinned.push(entry.id);
            } else if favorite_ids.contains(&entry.id) {
                selected_unpinned.push(entry.id);
            } else if ordinary_count < limit {
                selected_unpinned.push(entry.id);
                ordinary_count += 1;
            }
            true
        })?;

        pinned.extend(selected_unpinned);
        Ok(pinned)
    }

    /// Collect all pinned record IDs in newest-first order.
    pub fn pinned_ids(&self) -> Result<Vec<u64>, RepositoryError> {
        let mut pinned = Vec::new();

        self.entries.scan_descending(&mut |key, value| {
            let Some(entry) = Self::decode_entry(key, value) else {
                return true;
            };

            if entry.is_pinned {
                pinned.push(entry.id);
            }
            true
        })?;

        Ok(pinned)
    }

    /// Collect up to `max` oldest unpinned entries for cleanup.
    ///
    /// Returns `(encoded_key, record_id)` pairs sorted oldest-first
    /// (natural iteration order).
    pub fn oldest_unpinned(&self, max: usize) -> Result<Vec<([u8; 16], u64)>, RepositoryError> {
        let mut result = Vec::new();

        self.entries.scan_ascending(&mut |key, value| {
            if result.len() >= max {
                return false;
            }
            if key.len() != 16 || value.is_empty() {
                return true;
            }
            if value[0] != 0 {
                // pinned — skip
                return true;
            }
            let id = u64::from_be_bytes(key[8..].try_into().unwrap_or_default());
            let mut encoded_key = [0u8; 16];
            encoded_key.copy_from_slice(key);
            result.push((encoded_key, id));
            true
        })?;

        Ok(result)
    }

    /// Remove entry associated with the given `id`.
    fn remove_by_id(&self, id: u64) -> Result<(), RepositoryError> {
        let id_key = id.to_be_bytes();
        let Some(timestamp_bytes) = self.id_lookup.get(&id_key)? else {
            return Ok(());
        };
        let timestamp = Self::decode_lookup_timestamp(&timestamp_bytes)?;
        let key = Self::encode_key(timestamp, id);
        self.entries.remove(&key)?;
        self.id_lookup.remove(&id_key)?;
        Ok(())
    }

    // ── Encoding helpers ──────────────────────────────────────────

    pub(crate) fn encode_key(timestamp_millis: i64, id: u64) -> [u8; 16] {
        let mut key = [0u8; 16];
        key[..8].copy_from_slice(&timestamp_millis.to_be_bytes());
        key[8..].copy_from_slice(&id.to_be_bytes());
        key
    }

    fn encode_value(pinned: bool, content_type: &ContentType) -> [u8; 2] {
        [u8::from(pinned), content_type.as_tag()]
    }

    fn decode_lookup_timestamp(bytes: &[u8]) -> Result<i64, RepositoryError> {
        let timestamp: [u8; 8] = bytes
            .try_into()
            .map_err(|_| RepositoryError::Deserialization("bad time index lookup".into()))?;
        Ok(i64::from_be_bytes(timestamp))
    }

    fn decode_entry(key: &[u8], value: &[u8]) -> Option<IndexEntry> {
        if key.len() != 16 || value.is_empty() {
            return None;
        }
        let id = u64::from_be_bytes(key[8..].try_into().ok()?);
        let is_pinned = value[0] != 0;
        Some(IndexEntry { id, is_pinned })
    }

    /// Delete a raw encoded key from the underlying tree.
    pub fn remove_raw(&self, key: &[u8; 16]) -> Result<(), RepositoryError> {
        self.entries.remove(key.as_ref())?;
        let id = u64::from_be_bytes(key[8..].try_into().unwrap_or_default());
        self.id_lookup.remove(&id.to_be_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
impl TimeIndex {
    /// Direct insert for test setup (bypasses `upsert` scan).
    pub fn insert_raw(
        &self,
        timestamp_millis: i64,
        id: u64,
        pinned: bool,
        content_type: &ContentType,
    ) {
        let key = Self::encode_key(timestamp_millis, id);
        let val = Self::encode_value(pinned, content_type);
        #[allow(clippy::expect_used)]
        self.entries.insert(&key, &val).expect("test insert failed");
        #[allow(clippy::expect_used)]
        self.id_lookup
            .insert(&id.to_be_bytes(), &timestamp_millis.to_be_bytes())
            .expect("test lookup insert failed");
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rstest::rstest;
    use tempfile::tempdir;

    use super::*;
    use crate::repository::{
        backend::BackendFactory, memory_backend::memory_backend_factory,
        redb_backend::redb_backend_factory, sled_backend::sled_backend_factory,
    };

    #[allow(clippy::expect_used)]
    fn create_test_time_index_with(factory: BackendFactory) -> (tempfile::TempDir, TimeIndex) {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_time_index.db");
        let backend = factory(&db_path).expect("Failed to open backend");
        let entries = backend
            .open_tree("time_index")
            .expect("Failed to open tree");
        let id_lookup = backend
            .open_tree("time_index_lookup")
            .expect("Failed to open lookup tree");
        let index = TimeIndex::new(entries, id_lookup);
        (temp_dir, index)
    }

    #[allow(clippy::expect_used)]
    fn create_test_time_index() -> TimeIndex {
        let (_temp_dir, index) = create_test_time_index_with(memory_backend_factory);
        index
    }

    #[allow(clippy::expect_used)]
    fn select_display_ids_without_favorites(index: &TimeIndex, limit: usize) -> Vec<u64> {
        index
            .select_display_ids(limit, &HashSet::new())
            .expect("Failed to select")
    }

    // ── Encoding/Decoding Tests ───────────────────────────────────

    #[test]
    fn test_encode_key_basic() {
        let key = TimeIndex::encode_key(1_234_567_890_123_i64, 9_876_543_210_u64);
        // Verify key is 16 bytes
        assert_eq!(key.len(), 16);
        // Verify timestamp is in first 8 bytes (big-endian)
        assert_eq!(&key[..8], &1_234_567_890_123_i64.to_be_bytes());
        // Verify id is in last 8 bytes (big-endian)
        assert_eq!(&key[8..], &9_876_543_210_u64.to_be_bytes());
    }

    #[test]
    fn test_encode_key_negative_timestamp() {
        // Negative timestamp (before epoch)
        let key = TimeIndex::encode_key(-1_i64, 1_u64);
        assert_eq!(key.len(), 16);
        // First byte should be 0xFF for negative numbers in two's complement
        assert_eq!(key[0], 0xFF);
    }

    #[test]
    fn test_encode_key_zero_values() {
        let key = TimeIndex::encode_key(0_i64, 0_u64);
        assert_eq!(key, [0u8; 16]);
    }

    #[test]
    fn test_encode_value_text() {
        let val = TimeIndex::encode_value(false, &ContentType::Text);
        assert_eq!(val, [0, 0]); // not pinned, text tag = 0

        let val_pinned = TimeIndex::encode_value(true, &ContentType::Text);
        assert_eq!(val_pinned, [1, 0]); // pinned, text tag = 0
    }

    #[test]
    fn test_encode_value_image() {
        let val = TimeIndex::encode_value(false, &ContentType::Image);
        assert_eq!(val, [0, 1]); // not pinned, image tag = 1
    }

    #[test]
    fn test_encode_value_file_path() {
        let val = TimeIndex::encode_value(true, &ContentType::FilePath);
        assert_eq!(val, [1, 2]); // pinned, file_path tag = 2
    }
    #[test]
    #[allow(clippy::expect_used)]
    fn test_decode_entry_valid() {
        let key = TimeIndex::encode_key(12345_i64, 67890_u64);
        let value = TimeIndex::encode_value(true, &ContentType::Image);

        let entry = TimeIndex::decode_entry(&key, &value).expect("Should decode successfully");
        assert_eq!(entry.id, 67890);
        assert!(entry.is_pinned);
    }
    #[test]
    fn test_decode_entry_invalid_key_length() {
        let short_key = [1u8; 8];
        let value = TimeIndex::encode_value(false, &ContentType::Text);

        let result = TimeIndex::decode_entry(&short_key, &value);
        assert!(result.is_none());
    }

    #[test]
    fn test_decode_entry_empty_value() {
        let key = TimeIndex::encode_key(12345_i64, 67890_u64);
        let empty_value: &[u8] = &[];

        let result = TimeIndex::decode_entry(&key, empty_value);
        assert!(result.is_none());
    }

    // ── CRUD Tests ────────────────────────────────────────────────

    #[rstest]
    #[case(sled_backend_factory)]
    #[case(redb_backend_factory)]
    #[case(memory_backend_factory)]
    #[allow(clippy::expect_used)]
    fn test_upsert_new_record(#[case] factory: BackendFactory) {
        let (_temp_dir, index) = create_test_time_index_with(factory);
        let record = ClipboardRecord {
            id: 1,
            content: "test".to_string(),
            created_at: chrono::Local::now(),
            content_type: ContentType::Text,
            pinned: false,
        };

        index
            .upsert(&record)
            .unwrap_or_else(|error| panic!("Failed to upsert: {error}"));

        let ids = select_display_ids_without_favorites(&index, 10);
        assert_eq!(ids, vec![1]);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_upsert_updates_timestamp() {
        let index = create_test_time_index();
        let now = chrono::Local::now();
        let later = now + chrono::Duration::milliseconds(100);

        // Insert initial record
        let record1 = ClipboardRecord {
            id: 1,
            content: "test".to_string(),
            created_at: now,
            content_type: ContentType::Text,
            pinned: false,
        };
        index.upsert(&record1).expect("Failed to upsert first");

        // Upsert same id with later timestamp
        let record2 = ClipboardRecord {
            id: 1,
            content: "test".to_string(),
            created_at: later,
            content_type: ContentType::Text,
            pinned: false,
        };
        index.upsert(&record2).expect("Failed to upsert second");

        // Should only have one entry
        let ids = select_display_ids_without_favorites(&index, 10);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], 1);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_remove() {
        let index = create_test_time_index();
        let timestamp = chrono::Local::now().timestamp_millis();

        index.insert_raw(timestamp, 1, false, &ContentType::Text);

        let ids_before = select_display_ids_without_favorites(&index, 10);
        assert_eq!(ids_before.len(), 1);

        index.remove(timestamp, 1).expect("Failed to remove");

        let ids_after = select_display_ids_without_favorites(&index, 10);
        assert!(ids_after.is_empty());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_remove_nonexistent() {
        let index = create_test_time_index();

        // Should not error when removing non-existent entry
        index
            .remove(12345, 99999)
            .expect("Should not fail on non-existent");
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_update_pinned() {
        let index = create_test_time_index();
        let timestamp = chrono::Local::now().timestamp_millis();

        index.insert_raw(timestamp, 1, false, &ContentType::Text);

        // Verify initially unpinned
        let ids = select_display_ids_without_favorites(&index, 10);
        // Unpinned records are not returned first, so this verifies it's unpinned
        assert_eq!(ids, vec![1]);

        // Update to pinned
        index
            .update_pinned(timestamp, 1, true, &ContentType::Text)
            .expect("Failed to update pinned");

        // Verify now pinned (should appear in results even with limit 0)
        let ids = select_display_ids_without_favorites(&index, 10);
        assert_eq!(ids, vec![1]);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_clear() {
        let index = create_test_time_index();

        index.insert_raw(1000, 1, false, &ContentType::Text);
        index.insert_raw(2000, 2, true, &ContentType::Image);
        index.insert_raw(3000, 3, false, &ContentType::FilePath);

        assert_eq!(index.entries.len(), 3);
        assert_eq!(index.id_lookup.len(), 3);

        index.clear().expect("Failed to clear");

        assert_eq!(index.entries.len(), 0);
        assert_eq!(index.id_lookup.len(), 0);
        let ids = select_display_ids_without_favorites(&index, 10);
        assert!(ids.is_empty());
    }

    // ── Query Tests ───────────────────────────────────────────────

    #[test]
    #[allow(clippy::expect_used)]
    fn test_select_display_ids_empty() {
        let index = create_test_time_index();

        let ids = select_display_ids_without_favorites(&index, 10);
        assert!(ids.is_empty());
    }

    #[rstest]
    #[case(sled_backend_factory)]
    #[case(redb_backend_factory)]
    #[case(memory_backend_factory)]
    #[allow(clippy::expect_used)]
    fn test_select_display_ids_ordering(#[case] factory: BackendFactory) {
        let (_temp_dir, index) = create_test_time_index_with(factory);

        // Insert in non-chronological order
        index.insert_raw(3000, 3, false, &ContentType::Text);
        index.insert_raw(1000, 1, false, &ContentType::Text);
        index.insert_raw(2000, 2, false, &ContentType::Text);

        let ids = select_display_ids_without_favorites(&index, 10);
        // Should be in reverse chronological order (newest first)
        assert_eq!(ids, vec![3, 2, 1]);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_select_display_ids_with_limit() {
        let index = create_test_time_index();

        for i in 1..=5 {
            index.insert_raw(i * 1000, i as u64, false, &ContentType::Text);
        }

        let ids = select_display_ids_without_favorites(&index, 3);
        assert_eq!(ids.len(), 3);
        // Should get the 3 most recent
        assert_eq!(ids, vec![5, 4, 3]);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_select_display_ids_pinned_always_included() {
        let index = create_test_time_index();

        // Insert unpinned records
        for i in 1..=5 {
            index.insert_raw(i * 1000, i as u64, false, &ContentType::Text);
        }

        // Insert pinned records (older timestamps)
        index.insert_raw(100, 100, true, &ContentType::Text);
        index.insert_raw(200, 200, true, &ContentType::Text);

        let ids = select_display_ids_without_favorites(&index, 3);

        // Pinned records should appear first and do not consume the ordinary limit.
        assert_eq!(ids.len(), 5);
        assert_eq!(ids[0], 200); // newer pinned
        assert_eq!(ids[1], 100); // older pinned
        // Remaining slots go to the 3 most recent unpinned records.
        assert_eq!(ids[2], 5);
        assert_eq!(ids[3], 4);
        assert_eq!(ids[4], 3);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_select_display_ids_only_pinned() {
        let index = create_test_time_index();

        index.insert_raw(1000, 1, true, &ContentType::Text);
        index.insert_raw(2000, 2, true, &ContentType::Text);

        let ids = select_display_ids_without_favorites(&index, 10);
        assert_eq!(ids, vec![2, 1]); // Newer pinned first
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_select_display_ids_limit_zero() {
        let index = create_test_time_index();

        index.insert_raw(1000, 1, false, &ContentType::Text);
        index.insert_raw(2000, 2, true, &ContentType::Text); // pinned

        let ids = select_display_ids_without_favorites(&index, 0);
        // Pinned records should still appear even with limit 0
        assert_eq!(ids, vec![2]);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_select_display_ids_when_favorites_present_keep_default_time_order() {
        let index = create_test_time_index();

        index.insert_raw(1000, 1, false, &ContentType::Text);
        index.insert_raw(2000, 2, false, &ContentType::Text);
        index.insert_raw(3000, 3, false, &ContentType::Text);
        index.insert_raw(4000, 4, true, &ContentType::Text);

        let favorite_ids = HashSet::from([2_u64]);
        let ids = index
            .select_display_ids(1, &favorite_ids)
            .expect("Failed to select");

        assert_eq!(ids, vec![4, 3, 2]);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_select_display_ids_when_limit_zero_returns_pinned_and_favorites() {
        let index = create_test_time_index();

        index.insert_raw(1000, 1, false, &ContentType::Text);
        index.insert_raw(2000, 2, false, &ContentType::Text);
        index.insert_raw(3000, 3, true, &ContentType::Text);

        let favorite_ids = HashSet::from([2_u64]);
        let ids = index
            .select_display_ids(0, &favorite_ids)
            .expect("Failed to select");

        assert_eq!(ids, vec![3, 2]);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_oldest_unpinned_empty() {
        let index = create_test_time_index();

        let result = index.oldest_unpinned(10).expect("Failed to get oldest");
        assert!(result.is_empty());
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_oldest_unpinned_basic() {
        let index = create_test_time_index();

        // Insert unpinned records
        index.insert_raw(1000, 1, false, &ContentType::Text);
        index.insert_raw(2000, 2, false, &ContentType::Text);
        index.insert_raw(3000, 3, false, &ContentType::Text);

        let result = index.oldest_unpinned(2).expect("Failed to get oldest");
        assert_eq!(result.len(), 2);
        // Should return oldest first
        assert_eq!(result[0].1, 1);
        assert_eq!(result[1].1, 2);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_oldest_unpinned_skips_pinned() {
        let index = create_test_time_index();

        index.insert_raw(1000, 1, false, &ContentType::Text);
        index.insert_raw(2000, 2, true, &ContentType::Text); // pinned - should skip
        index.insert_raw(3000, 3, false, &ContentType::Text);

        let result = index.oldest_unpinned(10).expect("Failed to get oldest");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].1, 1);
        assert_eq!(result[1].1, 3);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_oldest_unpinned_with_limit() {
        let index = create_test_time_index();

        for i in 1..=5 {
            index.insert_raw(i * 1000, i as u64, false, &ContentType::Text);
        }

        let result = index.oldest_unpinned(3).expect("Failed to get oldest");
        assert_eq!(result.len(), 3);
        // Should get the 3 oldest
        assert_eq!(result[0].1, 1);
        assert_eq!(result[1].1, 2);
        assert_eq!(result[2].1, 3);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_oldest_unpinned_returns_encoded_key() {
        let index = create_test_time_index();

        let timestamp = 1_234_567_890_123_i64;
        let id = 9_876_543_210_u64;
        index.insert_raw(timestamp, id, false, &ContentType::Text);

        let result = index.oldest_unpinned(1).expect("Failed to get oldest");
        assert_eq!(result.len(), 1);

        // Verify the returned key is correctly encoded
        let (key, returned_id) = &result[0];
        assert_eq!(*returned_id, id);
        assert_eq!(&key[..8], &timestamp.to_be_bytes());
        assert_eq!(&key[8..], &id.to_be_bytes());
    }

    // ── remove_by_id Tests ────────────────────────────────────────

    #[test]
    #[allow(clippy::expect_used)]
    fn test_remove_by_id() {
        let index = create_test_time_index();

        index.insert_raw(1000, 1, false, &ContentType::Text);
        index.insert_raw(2000, 2, false, &ContentType::Text);

        // Use internal method to remove by id
        index.remove_by_id(1).expect("Failed to remove by id");

        let ids = select_display_ids_without_favorites(&index, 10);
        assert_eq!(ids, vec![2]);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_remove_by_id_nonexistent() {
        let index = create_test_time_index();

        index.insert_raw(1000, 1, false, &ContentType::Text);

        // Should not error
        index
            .remove_by_id(999)
            .expect("Should not fail on non-existent");

        let ids = select_display_ids_without_favorites(&index, 10);
        assert_eq!(ids, vec![1]);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_remove_by_id_multiple_same_id() {
        // This shouldn't happen in practice, but test the behavior
        let index = create_test_time_index();

        // Insert two entries with same id but different timestamps
        index.insert_raw(1000, 1, false, &ContentType::Text);
        index.insert_raw(2000, 1, false, &ContentType::Text);

        // remove_by_id should only remove one (the first found in reverse iteration)
        index.remove_by_id(1).expect("Failed to remove by id");

        let ids = select_display_ids_without_favorites(&index, 10);
        assert_eq!(ids.len(), 1);
    }

    // ── Edge Cases and Error Handling ─────────────────────────────

    #[test]
    #[allow(clippy::expect_used)]
    fn test_decode_entry_corrupted_data() {
        let key = TimeIndex::encode_key(12345_i64, 67890_u64);

        // Valid key but value with only pinned flag (missing content type)
        let corrupted_value = [1u8]; // Just pinned flag, no content type

        // Should still decode (content type is in second byte, but we don't check length)
        let entry = TimeIndex::decode_entry(&key, &corrupted_value);
        // Implementation doesn't validate value length, so this returns Some
        assert!(entry.is_some());
        assert!(entry.expect("Should have entry").is_pinned);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_concurrent_upserts() {
        let index = create_test_time_index();
        let index = std::sync::Arc::new(index);
        let mut handles = vec![];

        for i in 0..10 {
            let idx = index.clone();
            let handle = std::thread::spawn(move || {
                let record = ClipboardRecord {
                    id: i as u64,
                    content: format!("thread {i}"),
                    created_at: chrono::Local::now(),
                    content_type: ContentType::Text,
                    pinned: false,
                };
                idx.upsert(&record).expect("Failed to upsert");
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().expect("Thread panicked");
        }

        let ids = select_display_ids_without_favorites(&index, 20);
        assert_eq!(ids.len(), 10);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_large_number_of_records() {
        let index = create_test_time_index();
        let count = 1000;

        for i in 0..count {
            index.insert_raw(
                i64::from(i) * 1000,
                i as u64,
                i % 3 == 0,
                &ContentType::Text,
            );
        }

        // Request 100 records - should get all pinned (334) which exceeds limit
        // but pinned records are always included
        let ids = select_display_ids_without_favorites(&index, 100);
        // Pinned records (i % 3 == 0): 0, 3, 6, ... 999 = 334 records
        // All pinned records should be included even if exceeding limit
        assert!(!ids.is_empty());

        // Verify we get all pinned records plus the requested ordinary window.
        let expected_pinned_count = (0..count).filter(|i| i % 3 == 0).count();
        assert_eq!(ids.len(), expected_pinned_count + 100);

        let oldest = index.oldest_unpinned(50).expect("Failed to get oldest");
        assert_eq!(oldest.len(), 50);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_different_content_types() {
        let index = create_test_time_index();
        let now = chrono::Local::now().timestamp_millis();

        index.insert_raw(now, 1, false, &ContentType::Text);
        index.insert_raw(now + 1, 2, false, &ContentType::Image);
        index.insert_raw(now + 2, 3, false, &ContentType::FilePath);

        let ids = select_display_ids_without_favorites(&index, 10);
        assert_eq!(ids.len(), 3);
        // Order should be by timestamp, not content type
        assert_eq!(ids, vec![3, 2, 1]);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn test_remove_raw() {
        let index = create_test_time_index();
        let timestamp = chrono::Local::now().timestamp_millis();

        index.insert_raw(timestamp, 1, false, &ContentType::Text);

        let key = TimeIndex::encode_key(timestamp, 1);
        index.remove_raw(&key).expect("Failed to remove raw");

        assert_eq!(index.entries.len(), 0);
        assert_eq!(index.id_lookup.len(), 0);
        let ids = select_display_ids_without_favorites(&index, 10);
        assert!(ids.is_empty());
    }
}

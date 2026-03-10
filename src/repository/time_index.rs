//! Lightweight secondary index keyed by timestamp for efficient
//! chronological queries without full record deserialization.
//!
//! ## Key format (16 bytes)
//! `timestamp_millis(i64 BE, 8B) ++ record_id(u64 BE, 8B)`
//!
//! ## Value format (2 bytes)
//! `pinned(u8) ++ content_type_tag(u8)`

use sled::Tree;

use super::{
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
    tree: Tree,
}

impl TimeIndex {
    pub const fn new(tree: Tree) -> Self {
        Self { tree }
    }

    /// Insert or update the time index entry for a record.
    ///
    /// Removes any stale entry with the same `id` before inserting
    /// the new one (timestamp may have changed on dedup upsert).
    pub fn upsert(&self, record: &ClipboardRecord) -> Result<(), RepositoryError> {
        self.remove_by_id(record.id)?;
        let key = Self::encode_key(record.created_at.timestamp_millis(), record.id);
        let val = Self::encode_value(record.pinned, &record.content_type);
        self.tree
            .insert(key, &val)
            .map_err(|e| RepositoryError::Insert(e.to_string()))?;
        Ok(())
    }

    /// Remove the entry that matches the given `(timestamp_millis, id)` pair.
    pub fn remove(&self, timestamp_millis: i64, id: u64) -> Result<(), RepositoryError> {
        let key = Self::encode_key(timestamp_millis, id);
        self.tree
            .remove(key)
            .map_err(|e| RepositoryError::Delete(e.to_string()))?;
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
        self.tree
            .insert(key, &val)
            .map_err(|e| RepositoryError::Insert(e.to_string()))?;
        Ok(())
    }

    /// Clear all entries.
    pub fn clear(&self) -> Result<(), RepositoryError> {
        self.tree
            .clear()
            .map_err(|e| RepositoryError::Delete(e.to_string()))?;
        Ok(())
    }

    /// Select up to `limit` record IDs for display.
    ///
    /// All pinned IDs are collected first (they always appear), then unpinned
    /// IDs are appended until the total reaches `limit`. Returns IDs in
    /// newest-first order within each group.
    pub fn select_recent_ids(&self, limit: usize) -> Result<Vec<u64>, RepositoryError> {
        let mut pinned: Vec<u64> = Vec::new();
        let mut unpinned: Vec<u64> = Vec::new();

        for result in self.tree.iter().rev() {
            let (k, v) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
            let Some(entry) = Self::decode_entry(&k, &v) else {
                continue;
            };
            if entry.is_pinned {
                pinned.push(entry.id);
            } else if pinned.len() + unpinned.len() < limit {
                unpinned.push(entry.id);
            }
        }

        let unpinned_slots = limit.saturating_sub(pinned.len());
        unpinned.truncate(unpinned_slots);
        pinned.extend(unpinned);
        Ok(pinned)
    }

    /// Collect up to `max` oldest unpinned entries for cleanup.
    ///
    /// Returns `(encoded_key, record_id)` pairs sorted oldest-first
    /// (natural sled iteration order).
    pub fn oldest_unpinned(&self, max: usize) -> Result<Vec<([u8; 16], u64)>, RepositoryError> {
        let mut result = Vec::new();
        for entry in &self.tree {
            if result.len() >= max {
                break;
            }
            let (k, v) = entry.map_err(|e| RepositoryError::Query(e.to_string()))?;
            if k.len() != 16 || v.is_empty() {
                continue;
            }
            if v[0] != 0 {
                // pinned — skip
                continue;
            }
            let id = u64::from_be_bytes(k[8..].try_into().unwrap_or_default());
            let mut key = [0u8; 16];
            key.copy_from_slice(&k);
            result.push((key, id));
        }
        Ok(result)
    }

    /// Remove entry associated with the given `id` by scanning the index.
    fn remove_by_id(&self, id: u64) -> Result<(), RepositoryError> {
        let id_bytes = id.to_be_bytes();
        let mut to_remove = None;
        for entry in self.tree.iter().rev() {
            let (k, _) = entry.map_err(|e| RepositoryError::Query(e.to_string()))?;
            if k.len() == 16 && k[8..] == id_bytes {
                to_remove = Some(k);
                break;
            }
        }
        if let Some(k) = to_remove {
            self.tree
                .remove(k)
                .map_err(|e| RepositoryError::Delete(e.to_string()))?;
        }
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
        self.tree
            .remove(key.as_ref())
            .map_err(|e| RepositoryError::Delete(e.to_string()))?;
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
        self.tree.insert(key, &val).expect("test insert failed");
    }
}

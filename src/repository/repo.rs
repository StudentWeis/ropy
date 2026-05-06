#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Clipboard repository for storing and retrieving clipboard records.

use std::path::{Path, PathBuf};

use chrono::Local;

use super::{
    backend::{BackendFactory, KvTree, StorageBackend},
    errors::RepositoryError,
    models::{ClipboardRecord, ContentType},
    redb_backend::{RedbBackend, redb_backend_factory},
    sidecar::{
        purge_sidecar_dirs, remove_image_files, remove_record_sidecars,
        remove_superseded_rich_text_files,
    },
    time_index::TimeIndex,
};
use crate::{
    clipboard::save_rich_text_files_to_dir,
    utils::{content_hash, normalize_file_paths, serialize_file_paths},
};

/// Bump whenever the on-disk key/value layout changes — a mismatch wipes the
/// database on startup so stale records can't be misinterpreted.
const SCHEMA_VERSION: u64 = 3;

pub struct ClipboardRepository<B: StorageBackend = RedbBackend> {
    backend: B,
    pub(super) records: B::Tree,
    pub(super) time_index: TimeIndex<B::Tree>,
    pub(super) favorites: B::Tree,
    images_dir: PathBuf,
    rich_text_dir: PathBuf,
}

impl ClipboardRepository<RedbBackend> {
    pub fn new() -> Result<Self, RepositoryError> {
        let db_path = Self::default_db_path()?;
        let images_dir = dirs::data_local_dir()
            .ok_or(RepositoryError::DataDirNotFound)?
            .join("ropy")
            .join("images");
        Self::init(&db_path, images_dir, redb_backend_factory)
    }

    fn default_db_path() -> Result<PathBuf, RepositoryError> {
        Ok(dirs::data_local_dir()
            .ok_or(RepositoryError::DataDirNotFound)?
            .join("ropy")
            .join("clipboard.redb"))
    }
}

impl<B: StorageBackend> ClipboardRepository<B> {
    /// Build a repository against an explicitly chosen backend. Tests use
    /// this seam to inject the in-memory backend; production goes through
    /// [`ClipboardRepository::new`].
    pub fn init(
        db_path: &PathBuf,
        images_dir: PathBuf,
        factory: BackendFactory<B>,
    ) -> Result<Self, RepositoryError> {
        Self::from_backend(factory(db_path)?, images_dir)
    }

    pub fn from_backend(backend: B, images_dir: PathBuf) -> Result<Self, RepositoryError> {
        let meta = backend.open_tree("meta")?;
        let records = backend.open_tree("clipboard_records")?;
        let time_index = TimeIndex::new(
            backend.open_tree("time_index")?,
            backend.open_tree("time_index_lookup")?,
        );
        let favorites = backend.open_tree("favorites")?;

        let rich_text_dir = images_dir.parent().map_or_else(
            || PathBuf::from("rich_text"),
            |parent| parent.join("rich_text"),
        );

        if Self::needs_schema_migration(&meta)? {
            records.clear()?;
            time_index.clear()?;
            favorites.clear()?;
            purge_sidecar_dirs(&images_dir, &rich_text_dir);
            meta.insert(b"schema_version", &SCHEMA_VERSION.to_be_bytes())?;
            backend.flush()?;
        }

        Ok(Self {
            backend,
            records,
            time_index,
            favorites,
            images_dir,
            rich_text_dir,
        })
    }

    pub fn flush(&self) -> Result<(), RepositoryError> {
        self.backend.flush()
    }

    fn needs_schema_migration(meta: &impl KvTree) -> Result<bool, RepositoryError> {
        match meta.get(b"schema_version")? {
            Some(v) if v.len() == 8 => {
                let stored =
                    u64::from_be_bytes(v[..8].try_into().map_err(|_| {
                        RepositoryError::Deserialization("bad schema version".into())
                    })?);
                Ok(stored != SCHEMA_VERSION)
            }
            _ => Ok(true),
        }
    }
}

impl<B: StorageBackend> Drop for ClipboardRepository<B> {
    fn drop(&mut self) {
        self.flush().ok();
    }
}

impl<B: StorageBackend> ClipboardRepository<B> {
    /// Insert or refresh a record keyed by its content hash. A duplicate
    /// hash bumps `created_at` only — the existing payload is preserved so
    /// re-copies surface at the top of the board without churning storage.
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
            rich_text_meta: None,
        };
        self.put_raw(&key, &record)?;
        self.time_index.upsert(&record)?;
        Ok(record)
    }

    /// Persist an image record whose payload already lives at `file_path`.
    /// On a duplicate hash the new file is deleted to avoid leaking sidecar
    /// blobs, and only `created_at` is bumped on the existing record.
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
                remove_image_files(&file_path);
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
            rich_text_meta: None,
        };
        self.put_raw(&key, &record)?;
        self.time_index.upsert(&record)?;
        Ok(record)
    }

    pub fn save_text(&self, content: String) -> Result<ClipboardRecord, RepositoryError> {
        self.save(content, ContentType::Text)
    }

    /// Normalize and persist a file list. Errors when the list is empty so
    /// callers don't accidentally insert a record with no payload.
    pub fn save_files(&self, paths: &[String]) -> Result<ClipboardRecord, RepositoryError> {
        let normalized = normalize_file_paths(paths);
        if normalized.is_empty() {
            return Err(RepositoryError::Query("file list is empty".to_string()));
        }

        let content = serialize_file_paths(&normalized)
            .map_err(|error| RepositoryError::Serialization(error.to_string()))?;

        self.save(content, ContentType::FilePath)
    }

    pub fn save_rich_text(
        &self,
        plain_text: String,
        html: Option<&str>,
        rtf: Option<&str>,
    ) -> Result<ClipboardRecord, RepositoryError> {
        let id = content_hash(&plain_text, &ContentType::RichText);
        let key = id.to_be_bytes();
        let now = Local::now();
        let rich_text_meta = save_rich_text_files_to_dir(id, html, rtf, self.rich_text_root());

        if let Some(existing) = self.get_raw(&key)? {
            let mut record: ClipboardRecord = postcard::from_bytes(&existing)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            record.created_at = now;
            if let Some(meta) = rich_text_meta {
                remove_superseded_rich_text_files(record.rich_text_meta.as_ref(), &meta);
                record.rich_text_meta = Some(meta);
            }
            self.put_raw(&key, &record)?;
            self.time_index.upsert(&record)?;
            return Ok(record);
        }

        let record = ClipboardRecord {
            id,
            content: plain_text,
            created_at: now,
            content_type: ContentType::RichText,
            pinned: false,
            rich_text_meta,
        };
        self.put_raw(&key, &record)?;
        self.time_index.upsert(&record)?;
        Ok(record)
    }
}

impl<B: StorageBackend> ClipboardRepository<B> {
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

    pub fn count(&self) -> usize {
        self.records.len()
    }
}

impl<B: StorageBackend> ClipboardRepository<B> {
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

    pub fn delete(&self, id: u64) -> Result<bool, RepositoryError> {
        let record = self.get_by_id(id)?;
        if let Some(ref rec) = record {
            remove_record_sidecars(rec);
        }

        let key = id.to_be_bytes();
        let removed = self.records.remove(&key)?;
        self.remove_favorite(id)?;

        if let Some(rec) = record {
            self.time_index
                .remove(rec.created_at.timestamp_millis(), rec.id)?;
        }
        Ok(removed)
    }

    /// Wipe every record and its on-disk sidecars (images + rich text).
    pub fn clear(&self) -> Result<(), RepositoryError> {
        self.records.clear()?;
        self.time_index.clear()?;
        self.favorites.clear()?;
        purge_sidecar_dirs(&self.images_dir, &self.rich_text_dir);
        Ok(())
    }
}

impl<B: StorageBackend> ClipboardRepository<B> {
    pub(super) fn get_raw(&self, key: &[u8]) -> Result<Option<Vec<u8>>, RepositoryError> {
        self.records.get(key)
    }

    pub(super) fn put_raw(
        &self,
        key: &[u8],
        record: &ClipboardRecord,
    ) -> Result<(), RepositoryError> {
        let value = postcard::to_allocvec(record)
            .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
        self.records.insert(key, &value)
    }

    /// Load records by id, dropping (with a warn log) any that fail to
    /// deserialize so a single corrupt entry can't break list rendering.
    pub(super) fn load_records(&self, ids: &[u64]) -> Vec<ClipboardRecord> {
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

    pub(super) fn rich_text_root(&self) -> &Path {
        self.rich_text_dir
            .parent()
            .unwrap_or(self.rich_text_dir.as_path())
    }

    pub(super) fn decode_u64_key(bytes: &[u8]) -> Option<u64> {
        let key: [u8; 8] = bytes.try_into().ok()?;
        Some(u64::from_be_bytes(key))
    }
}

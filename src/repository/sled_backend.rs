//! Sled-based implementation of the [`StorageBackend`] trait.
//!
//! All `sled` crate dependencies are confined to this module so that
//! swapping the database only requires providing an alternative
//! [`StorageBackend`] implementation.

use std::path::PathBuf;

use sled::Db;

use super::{
    backend::{KvTree, StorageBackend},
    errors::RepositoryError,
};

/// Maximum page cache size for the sled database (8 MB).
///
/// sled defaults to 1 GB, which is far too large for a clipboard manager.
const DB_CACHE_CAPACITY: u64 = 8 * 1024 * 1024;

/// Interval at which sled flushes dirty pages to disk (in milliseconds).
const DB_FLUSH_INTERVAL_MS: u64 = 1000;

/// A [`StorageBackend`] backed by the sled embedded database.
pub struct SledBackend {
    db: Db,
}

impl SledBackend {
    /// Open a sled database at the given path with tuned defaults.
    pub fn open(db_path: &PathBuf) -> Result<Self, RepositoryError> {
        let db = sled::Config::new()
            .path(db_path)
            .cache_capacity(DB_CACHE_CAPACITY)
            .flush_every_ms(Some(DB_FLUSH_INTERVAL_MS))
            .open()
            .map_err(|e| RepositoryError::DatabaseOpen(e.to_string()))?;
        Ok(Self { db })
    }
}

impl StorageBackend for SledBackend {
    fn open_tree(&self, name: &str) -> Result<Box<dyn KvTree>, RepositoryError> {
        let tree = self
            .db
            .open_tree(name)
            .map_err(|e| RepositoryError::TreeOpen(e.to_string()))?;
        Ok(Box::new(SledTree(tree)))
    }

    fn flush(&self) -> Result<(), RepositoryError> {
        self.db
            .flush()
            .map_err(|e| RepositoryError::Flush(e.to_string()))?;
        Ok(())
    }
}

/// Factory function that creates a [`SledBackend`].
///
/// Pass this to [`ClipboardRepository::init`](super::ClipboardRepository::init)
/// when you want to use sled as the storage engine.
pub fn sled_backend_factory(db_path: &PathBuf) -> Result<Box<dyn StorageBackend>, RepositoryError> {
    Ok(Box::new(SledBackend::open(db_path)?))
}

/// Thin wrapper around [`sled::Tree`] implementing [`KvTree`].
struct SledTree(sled::Tree);

impl KvTree for SledTree {
    fn insert(&self, key: &[u8], value: &[u8]) -> Result<(), RepositoryError> {
        self.0
            .insert(key, value)
            .map_err(|e| RepositoryError::Insert(e.to_string()))?;
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, RepositoryError> {
        self.0
            .get(key)
            .map(|opt| opt.map(|ivec| ivec.to_vec()))
            .map_err(|e| RepositoryError::Query(e.to_string()))
    }

    fn remove(&self, key: &[u8]) -> Result<bool, RepositoryError> {
        self.0
            .remove(key)
            .map(|opt| opt.is_some())
            .map_err(|e| RepositoryError::Delete(e.to_string()))
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn clear(&self) -> Result<(), RepositoryError> {
        self.0
            .clear()
            .map_err(|e| RepositoryError::Delete(e.to_string()))
    }

    fn scan_ascending(
        &self,
        callback: &mut dyn FnMut(&[u8], &[u8]) -> bool,
    ) -> Result<(), RepositoryError> {
        for entry in &self.0 {
            let (key, value) = entry.map_err(|e| RepositoryError::Query(e.to_string()))?;
            if !callback(&key, &value) {
                break;
            }
        }
        Ok(())
    }

    fn scan_descending(
        &self,
        callback: &mut dyn FnMut(&[u8], &[u8]) -> bool,
    ) -> Result<(), RepositoryError> {
        for entry in self.0.iter().rev() {
            let (key, value) = entry.map_err(|e| RepositoryError::Query(e.to_string()))?;
            if !callback(&key, &value) {
                break;
            }
        }
        Ok(())
    }
}

//! Abstract storage backend trait for the clipboard repository.
//!
//! This module defines the [`StorageBackend`] trait that decouples the
//! repository's business logic from any concrete database implementation.
//! To swap the underlying database (e.g. from `sled` to `redb`), implement
//! this trait and pass it to [`ClipboardRepository`](super::ClipboardRepository).

use std::path::PathBuf;

use super::errors::RepositoryError;

/// A named, ordered key-value store that supports forward and reverse
/// iteration.  Each "tree" in the backend corresponds to a logical
/// namespace (e.g. `clipboard_records`, `time_index`, `favorites`).
pub trait KvTree: Send + Sync {
    /// Insert a key-value pair, overwriting any previous value.
    fn insert(&self, key: &[u8], value: &[u8]) -> Result<(), RepositoryError>;

    /// Get the value associated with a key, if it exists.
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, RepositoryError>;

    /// Remove a key-value pair, returning `true` if the key existed.
    fn remove(&self, key: &[u8]) -> Result<bool, RepositoryError>;

    /// Return the number of entries in this tree.
    fn len(&self) -> usize;

    /// Remove all entries.
    fn clear(&self) -> Result<(), RepositoryError>;

    /// Iterate over all entries in ascending key order.
    ///
    /// The callback returns `true` to continue, `false` to stop.
    fn scan_ascending(
        &self,
        callback: &mut dyn FnMut(&[u8], &[u8]) -> bool,
    ) -> Result<(), RepositoryError>;

    /// Iterate over all entries in descending key order.
    ///
    /// The callback returns `true` to continue, `false` to stop.
    fn scan_descending(
        &self,
        callback: &mut dyn FnMut(&[u8], &[u8]) -> bool,
    ) -> Result<(), RepositoryError>;
}

/// The top-level storage backend that manages multiple named trees and
/// database-level operations (flush, schema migration, etc.).
pub trait StorageBackend: Send + Sync {
    /// Open (or create) a named tree / namespace.
    fn open_tree(&self, name: &str) -> Result<Box<dyn KvTree>, RepositoryError>;

    /// Flush all pending writes to durable storage.
    fn flush(&self) -> Result<(), RepositoryError>;
}

/// Factory function type for creating a [`StorageBackend`] from a path.
///
/// Different backends can provide their own factory that is passed to
/// [`ClipboardRepository::init`](super::ClipboardRepository::init).
pub type BackendFactory = fn(&PathBuf) -> Result<Box<dyn StorageBackend>, RepositoryError>;

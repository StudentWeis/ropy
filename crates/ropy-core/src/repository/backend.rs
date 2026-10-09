//! Abstract storage backend trait for the clipboard repository.
//!
//! This module defines the `StorageBackend` trait that decouples the
//! repository's business logic from any concrete database implementation.
//! Production uses `redb`; tests can inject the in-memory backend through
//! this trait and the repository test helpers.

use std::path::PathBuf;

use super::errors::RepositoryError;

pub(super) const META_TREE: &str = "meta";
pub(super) const RECORDS_TREE: &str = "clipboard_records";
pub(super) const TIME_INDEX_TREE: &str = "time_index";
pub(super) const TIME_INDEX_LOOKUP_TREE: &str = "time_index_lookup";
pub(super) const FAVORITES_TREE: &str = "favorites";

/// A key qualified by its repository tree name.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct TreeKey<'a> {
    /// Target tree namespace.
    pub tree: &'static str,
    /// Encoded key bytes.
    pub key: &'a [u8],
}

impl<'a> TreeKey<'a> {
    /// Qualify encoded key bytes with a tree namespace.
    #[must_use]
    pub const fn new(tree: &'static str, key: &'a [u8]) -> Self {
        Self { tree, key }
    }
}

/// Ordered mutations committed together, or all discarded on failure.
#[derive(Debug)]
pub enum TreeWrite<'a> {
    /// Insert or replace the value at this key.
    Insert(TreeKey<'a>, &'a [u8]),
    /// Remove the value at this key, if present.
    Remove(TreeKey<'a>),
}

/// In-memory fault-injection backend for application tests.
#[cfg(any(test, feature = "test"))]
pub mod memory;
/// Persistent redb storage implementation.
pub mod redb;

/// A named, ordered key-value store with forward and reverse iteration.
/// Each "tree" is a logical namespace (e.g. `clipboard_records`,
/// `time_index`, `favorites`).
pub trait KvTree: Send + Sync {
    /// Insert or replace one encoded value.
    ///
    /// # Errors
    /// Returns an error if the write cannot be committed.
    fn insert(&self, key: &[u8], value: &[u8]) -> Result<(), RepositoryError>;

    /// Read an encoded value, returning `None` for an absent key.
    ///
    /// # Errors
    /// Returns an error if the backend cannot read the key.
    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, RepositoryError>;

    /// Returns `true` if the key existed before removal.
    ///
    /// # Errors
    /// Returns an error if the deletion cannot be committed.
    fn remove(&self, key: &[u8]) -> Result<bool, RepositoryError>;

    /// Return the number of entries currently visible in the tree.
    fn len(&self) -> usize;

    /// Whether this tree currently contains no entries.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Remove all entries from this test tree.
    ///
    /// # Errors
    /// Returns an error if the backend cannot commit the clear operation.
    #[cfg(test)]
    fn clear(&self) -> Result<(), RepositoryError>;

    /// Iterate ascending by key. The callback returns `false` to stop early.
    ///
    /// # Errors
    /// Returns an error if the backend cannot iterate or read entries.
    fn scan_ascending(
        &self,
        callback: &mut dyn FnMut(&[u8], &[u8]) -> bool,
    ) -> Result<(), RepositoryError>;

    /// Iterate descending by key. The callback returns `false` to stop early.
    ///
    /// # Errors
    /// Returns an error if the backend cannot iterate or read entries.
    fn scan_descending(
        &self,
        callback: &mut dyn FnMut(&[u8], &[u8]) -> bool,
    ) -> Result<(), RepositoryError>;
}

/// Top-level backend managing multiple [`KvTree`]s and database-level
/// operations (flush, schema migration, etc.).
pub trait StorageBackend: Send + Sync {
    /// Handle used to access a named tree.
    type Tree: KvTree;

    /// Open or create a named tree.
    ///
    /// # Errors
    /// Returns an error if the backend cannot open or create the tree.
    fn open_tree(&self, name: &str) -> Result<Self::Tree, RepositoryError>;

    /// Commit all ordered writes atomically or discard all of them.
    ///
    /// # Errors
    /// Returns an error if any write or the transaction commit fails; no partial batch is applied.
    fn write_batch(&self, writes: &[TreeWrite<'_>]) -> Result<(), RepositoryError>;

    /// Remove keys from multiple logical trees in one atomic transaction.
    ///
    /// # Errors
    /// Returns an error if the deletion transaction cannot be committed.
    fn remove_batch(&self, removals: &[TreeKey<'_>]) -> Result<Vec<bool>, RepositoryError>;

    /// Clear multiple logical trees in one atomic transaction.
    ///
    /// # Errors
    /// Returns an error if the clear transaction cannot be committed.
    fn clear_batch(&self, trees: &[&'static str]) -> Result<(), RepositoryError>;

    /// Flush pending storage changes to the backend.
    ///
    /// # Errors
    /// Returns an error if the backend cannot flush pending writes.
    fn flush(&self) -> Result<(), RepositoryError>;
}

/// Factory used by [`ClipboardRepository::init`](super::ClipboardRepository::init)
/// so tests can swap in alternative backends (e.g. the in-memory adapter).
pub type BackendFactory<B> = fn(&PathBuf) -> Result<B, RepositoryError>;

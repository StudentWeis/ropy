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

#[derive(Debug, Clone, Copy)]
pub(crate) struct TreeKey<'a> {
    pub tree: &'static str,
    pub key: &'a [u8],
}

impl<'a> TreeKey<'a> {
    pub(crate) const fn new(tree: &'static str, key: &'a [u8]) -> Self {
        Self { tree, key }
    }
}

#[cfg(test)]
pub mod memory;
pub mod redb;

/// A named, ordered key-value store with forward and reverse iteration.
/// Each "tree" is a logical namespace (e.g. `clipboard_records`,
/// `time_index`, `favorites`).
pub(crate) trait KvTree: Send + Sync {
    fn insert(&self, key: &[u8], value: &[u8]) -> Result<(), RepositoryError>;

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, RepositoryError>;

    /// Returns `true` if the key existed before removal.
    fn remove(&self, key: &[u8]) -> Result<bool, RepositoryError>;

    fn len(&self) -> usize;

    #[cfg(test)]
    fn clear(&self) -> Result<(), RepositoryError>;

    /// Iterate ascending by key. The callback returns `false` to stop early.
    fn scan_ascending(
        &self,
        callback: &mut dyn FnMut(&[u8], &[u8]) -> bool,
    ) -> Result<(), RepositoryError>;

    /// Iterate descending by key. The callback returns `false` to stop early.
    fn scan_descending(
        &self,
        callback: &mut dyn FnMut(&[u8], &[u8]) -> bool,
    ) -> Result<(), RepositoryError>;
}

/// Top-level backend managing multiple [`KvTree`]s and database-level
/// operations (flush, schema migration, etc.).
pub(crate) trait StorageBackend: Send + Sync {
    type Tree: KvTree;

    fn open_tree(&self, name: &str) -> Result<Self::Tree, RepositoryError>;

    /// Remove keys from multiple logical trees in one atomic transaction.
    fn remove_batch(&self, removals: &[TreeKey<'_>]) -> Result<Vec<bool>, RepositoryError>;

    /// Clear multiple logical trees in one atomic transaction.
    fn clear_batch(&self, trees: &[&'static str]) -> Result<(), RepositoryError>;

    fn flush(&self) -> Result<(), RepositoryError>;
}

/// Factory used by [`ClipboardRepository::init`](super::ClipboardRepository::init)
/// so tests can swap in alternative backends (e.g. the in-memory adapter).
pub(crate) type BackendFactory<B> = fn(&PathBuf) -> Result<B, RepositoryError>;

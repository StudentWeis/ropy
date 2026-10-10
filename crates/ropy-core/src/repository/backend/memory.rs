#![allow(clippy::significant_drop_tightening)]

//! In-memory implementation of the repository storage backend used by tests.

use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{
        Arc, LockResult, Mutex, PoisonError, RwLock,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::repository::{
    backend::{KvTree, StorageBackend, TreeKey, TreeWrite},
    errors::RepositoryError,
};

/// Shared in-memory storage with atomic batch failure injection for tests.
#[derive(Debug, Clone, Default)]
pub struct MemoryBackend {
    trees: Arc<Mutex<HashMap<String, Arc<MemoryTree>>>>,
    transaction_lock: Arc<Mutex<()>>,
    fail_next_batch: Arc<AtomicBool>,
}

impl MemoryBackend {
    /// Create empty in-memory storage.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Make the next batch mutation fail before changing any entries.
    pub fn fail_next_batch(&self) {
        self.fail_next_batch.store(true, Ordering::SeqCst);
    }
}

fn recover_lock<T>(result: LockResult<T>) -> T {
    result.unwrap_or_else(PoisonError::into_inner)
}

impl StorageBackend for MemoryBackend {
    type Tree = MemoryTreeHandle;

    fn open_tree(&self, name: &str) -> Result<Self::Tree, RepositoryError> {
        let trees_lock = self.trees.lock();
        let mut trees = recover_lock(trees_lock);
        let tree = trees
            .entry(name.to_string())
            .or_insert_with(|| Arc::new(MemoryTree::default()))
            .clone();
        Ok(MemoryTreeHandle {
            tree,
            transaction_lock: self.transaction_lock.clone(),
        })
    }

    fn remove_batch(&self, removals: &[TreeKey<'_>]) -> Result<Vec<bool>, RepositoryError> {
        let _transaction = recover_lock(self.transaction_lock.lock());
        if self.fail_next_batch.swap(false, Ordering::SeqCst) {
            return Err(RepositoryError::Delete(
                "injected batch deletion failure".to_string(),
            ));
        }

        let trees = recover_lock(self.trees.lock());
        let mut results = Vec::with_capacity(removals.len());
        for removal in removals {
            let removed = trees.get(removal.tree).is_some_and(|tree| {
                recover_lock(tree.entries.write())
                    .remove(removal.key)
                    .is_some()
            });
            results.push(removed);
        }
        Ok(results)
    }

    fn write_batch(&self, writes: &[TreeWrite<'_>]) -> Result<(), RepositoryError> {
        let _transaction = recover_lock(self.transaction_lock.lock());
        if self.fail_next_batch.swap(false, Ordering::SeqCst) {
            return Err(RepositoryError::Insert(
                "injected transaction failure".into(),
            ));
        }
        let mut trees = recover_lock(self.trees.lock());
        for write in writes {
            let key = match write {
                TreeWrite::Insert(key, _) | TreeWrite::Remove(key) => key,
            };
            let tree = trees.entry(key.tree.to_string()).or_default();
            let mut entries = recover_lock(tree.entries.write());
            match write {
                TreeWrite::Insert(_, value) => {
                    entries.insert(key.key.to_vec(), value.to_vec());
                }
                TreeWrite::Remove(_) => {
                    entries.remove(key.key);
                }
            }
        }
        Ok(())
    }

    fn clear_batch(&self, tree_names: &[&'static str]) -> Result<(), RepositoryError> {
        let _transaction = recover_lock(self.transaction_lock.lock());
        if self.fail_next_batch.swap(false, Ordering::SeqCst) {
            return Err(RepositoryError::Delete(
                "injected batch deletion failure".to_string(),
            ));
        }

        let trees = recover_lock(self.trees.lock());
        for tree_name in tree_names {
            if let Some(tree) = trees.get(*tree_name) {
                recover_lock(tree.entries.write()).clear();
            }
        }
        Ok(())
    }

    fn flush(&self) -> Result<(), RepositoryError> {
        Ok(())
    }
}

/// Create an in-memory backend using the repository factory signature.
/// The database path is ignored.
///
/// # Errors
/// This implementation always succeeds; the result matches the backend factory contract.
pub fn memory_backend_factory(_db_path: &PathBuf) -> Result<MemoryBackend, RepositoryError> {
    Ok(MemoryBackend::new())
}

#[derive(Debug, Default)]
struct MemoryTree {
    entries: RwLock<BTreeMap<Vec<u8>, Vec<u8>>>,
    fail_next_get: AtomicBool,
}

/// Shared handle to a tree guarded by the backend transaction lock.
#[derive(Debug, Clone)]
pub struct MemoryTreeHandle {
    tree: Arc<MemoryTree>,
    transaction_lock: Arc<Mutex<()>>,
}

impl MemoryTreeHandle {
    /// Make the next key read fail without changing stored data.
    pub fn fail_next_get(&self) {
        self.tree.fail_next_get.store(true, Ordering::SeqCst);
    }
}

impl KvTree for MemoryTreeHandle {
    fn insert(&self, key: &[u8], value: &[u8]) -> Result<(), RepositoryError> {
        let _transaction = recover_lock(self.transaction_lock.lock());
        let entries_lock = self.tree.entries.write();
        let mut entries = recover_lock(entries_lock);
        entries.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, RepositoryError> {
        if self.tree.fail_next_get.swap(false, Ordering::SeqCst) {
            return Err(RepositoryError::Query("injected key read failure".into()));
        }
        let entries_lock = self.tree.entries.read();
        let entries = recover_lock(entries_lock);
        Ok(entries.get(key).cloned())
    }

    fn remove(&self, key: &[u8]) -> Result<bool, RepositoryError> {
        let _transaction = recover_lock(self.transaction_lock.lock());
        let entries_lock = self.tree.entries.write();
        let mut entries = recover_lock(entries_lock);
        Ok(entries.remove(key).is_some())
    }

    fn len(&self) -> usize {
        let entries_lock = self.tree.entries.read();
        let entries = recover_lock(entries_lock);
        entries.len()
    }

    #[cfg(test)]
    fn clear(&self) -> Result<(), RepositoryError> {
        let _transaction = recover_lock(self.transaction_lock.lock());
        let entries_lock = self.tree.entries.write();
        let mut entries = recover_lock(entries_lock);
        entries.clear();
        Ok(())
    }

    fn scan_ascending(
        &self,
        callback: &mut dyn FnMut(&[u8], &[u8]) -> bool,
    ) -> Result<(), RepositoryError> {
        let entries_lock = self.tree.entries.read();
        let entries = recover_lock(entries_lock);
        for (key, value) in entries.iter() {
            if !callback(key, value) {
                break;
            }
        }
        Ok(())
    }

    fn scan_descending(
        &self,
        callback: &mut dyn FnMut(&[u8], &[u8]) -> bool,
    ) -> Result<(), RepositoryError> {
        let entries_lock = self.tree.entries.read();
        let entries = recover_lock(entries_lock);
        for (key, value) in entries.iter().rev() {
            if !callback(key, value) {
                break;
            }
        }
        Ok(())
    }
}

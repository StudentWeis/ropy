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
    backend::{KvTree, StorageBackend, TreeKey},
    errors::RepositoryError,
};

#[derive(Clone, Default)]
pub(crate) struct MemoryBackend {
    trees: Arc<Mutex<HashMap<String, Arc<MemoryTree>>>>,
    transaction_lock: Arc<Mutex<()>>,
    fail_next_batch: Arc<AtomicBool>,
}

impl MemoryBackend {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn fail_next_batch(&self) {
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

#[expect(clippy::unnecessary_wraps)]
pub(crate) fn memory_backend_factory(_db_path: &PathBuf) -> Result<MemoryBackend, RepositoryError> {
    Ok(MemoryBackend::new())
}

#[derive(Default)]
struct MemoryTree {
    entries: RwLock<BTreeMap<Vec<u8>, Vec<u8>>>,
}

#[derive(Clone)]
pub(crate) struct MemoryTreeHandle {
    tree: Arc<MemoryTree>,
    transaction_lock: Arc<Mutex<()>>,
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

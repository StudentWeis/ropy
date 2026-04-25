#![allow(clippy::significant_drop_tightening)]

//! In-memory implementation of the repository storage backend used by tests.

use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    sync::{Arc, LockResult, Mutex, PoisonError, RwLock},
};

use super::{
    backend::{KvTree, StorageBackend},
    errors::RepositoryError,
};

#[derive(Default)]
pub struct MemoryBackend {
    trees: Mutex<HashMap<String, Arc<MemoryTree>>>,
}

impl MemoryBackend {
    pub fn new() -> Self {
        Self::default()
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
        Ok(MemoryTreeHandle(tree))
    }

    fn flush(&self) -> Result<(), RepositoryError> {
        Ok(())
    }
}

#[allow(clippy::unnecessary_wraps)]
pub fn memory_backend_factory(_db_path: &PathBuf) -> Result<MemoryBackend, RepositoryError> {
    Ok(MemoryBackend::new())
}

#[derive(Default)]
struct MemoryTree {
    entries: RwLock<BTreeMap<Vec<u8>, Vec<u8>>>,
}

#[derive(Clone)]
pub struct MemoryTreeHandle(Arc<MemoryTree>);

impl KvTree for MemoryTreeHandle {
    fn insert(&self, key: &[u8], value: &[u8]) -> Result<(), RepositoryError> {
        let entries_lock = self.0.entries.write();
        let mut entries = recover_lock(entries_lock);
        entries.insert(key.to_vec(), value.to_vec());
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, RepositoryError> {
        let entries_lock = self.0.entries.read();
        let entries = recover_lock(entries_lock);
        Ok(entries.get(key).cloned())
    }

    fn remove(&self, key: &[u8]) -> Result<bool, RepositoryError> {
        let entries_lock = self.0.entries.write();
        let mut entries = recover_lock(entries_lock);
        Ok(entries.remove(key).is_some())
    }

    fn len(&self) -> usize {
        let entries_lock = self.0.entries.read();
        let entries = recover_lock(entries_lock);
        entries.len()
    }

    fn clear(&self) -> Result<(), RepositoryError> {
        let entries_lock = self.0.entries.write();
        let mut entries = recover_lock(entries_lock);
        entries.clear();
        Ok(())
    }

    fn scan_ascending(
        &self,
        callback: &mut dyn FnMut(&[u8], &[u8]) -> bool,
    ) -> Result<(), RepositoryError> {
        let entries_lock = self.0.entries.read();
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
        let entries_lock = self.0.entries.read();
        let entries = recover_lock(entries_lock);
        for (key, value) in entries.iter().rev() {
            if !callback(key, value) {
                break;
            }
        }
        Ok(())
    }
}

//! redb-based implementation of the [`StorageBackend`] trait.
//!
//! All `redb` crate dependencies are confined to this module so that
//! swapping the database only requires providing an alternative
//! [`StorageBackend`] implementation.

use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use redb::{Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};

use crate::repository::{
    backend::{KvTree, StorageBackend, TreeKey},
    errors::RepositoryError,
};

/// Maximum page cache size for the redb database (8 MB).
const DB_CACHE_CAPACITY: usize = 8 * 1024 * 1024;

type RedbTableDefinition<'a> = TableDefinition<'a, &'static [u8], &'static [u8]>;

/// A [`StorageBackend`] backed by the redb embedded database.
pub(crate) struct RedbBackend {
    db: Arc<Database>,
}

impl RedbBackend {
    /// Open a redb database at the given path with tuned defaults.
    pub(crate) fn open(db_path: &PathBuf) -> Result<Self, RepositoryError> {
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| RepositoryError::DatabaseOpen(error.to_string()))?;
        }

        let mut builder = redb::Builder::new();
        builder.set_cache_size(DB_CACHE_CAPACITY);
        let db = builder
            .create(db_path)
            .map_err(|error| RepositoryError::DatabaseOpen(error.to_string()))?;

        Ok(Self { db: Arc::new(db) })
    }
}

impl StorageBackend for RedbBackend {
    type Tree = RedbTree;

    fn open_tree(&self, name: &str) -> Result<Self::Tree, RepositoryError> {
        RedbTree::open(self.db.clone(), name.to_string())
    }

    fn remove_batch(&self, removals: &[TreeKey<'_>]) -> Result<Vec<bool>, RepositoryError> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|error| RepositoryError::Delete(error.to_string()))?;
        let mut results = Vec::with_capacity(removals.len());
        for removal in removals {
            let removed = {
                let mut table = write_txn
                    .open_table(RedbTree::table_definition(removal.tree))
                    .map_err(|error| RepositoryError::Delete(error.to_string()))?;
                table
                    .remove(removal.key)
                    .map_err(|error| RepositoryError::Delete(error.to_string()))?
                    .is_some()
            };
            results.push(removed);
        }
        write_txn
            .commit()
            .map_err(|error| RepositoryError::Delete(error.to_string()))?;
        Ok(results)
    }

    fn clear_batch(&self, trees: &[&'static str]) -> Result<(), RepositoryError> {
        let write_txn = self
            .db
            .begin_write()
            .map_err(|error| RepositoryError::Delete(error.to_string()))?;
        for tree_name in trees {
            let mut table = write_txn
                .open_table(RedbTree::table_definition(tree_name))
                .map_err(|error| RepositoryError::Delete(error.to_string()))?;
            table
                .retain(|_, _| false)
                .map_err(|error| RepositoryError::Delete(error.to_string()))?;
        }
        write_txn
            .commit()
            .map_err(|error| RepositoryError::Delete(error.to_string()))
    }

    fn flush(&self) -> Result<(), RepositoryError> {
        // redb writes are durably committed per transaction by default.
        Ok(())
    }
}

/// Factory function that creates a [`RedbBackend`].
pub(crate) fn redb_backend_factory(db_path: &PathBuf) -> Result<RedbBackend, RepositoryError> {
    RedbBackend::open(db_path)
}

pub(crate) struct RedbTree {
    db: Arc<Database>,
    name: String,
    write_lock: Mutex<()>,
}

impl RedbTree {
    fn open(db: Arc<Database>, name: String) -> Result<Self, RepositoryError> {
        let write_txn = db
            .begin_write()
            .map_err(|error| RepositoryError::TreeOpen(error.to_string()))?;
        {
            let table = write_txn
                .open_table(Self::table_definition(&name))
                .map_err(|error| RepositoryError::TreeOpen(error.to_string()))?;
            table
                .len()
                .map_err(|error| RepositoryError::TreeOpen(error.to_string()))?;
        }
        write_txn
            .commit()
            .map_err(|error| RepositoryError::TreeOpen(error.to_string()))?;

        Ok(Self {
            db,
            name,
            write_lock: Mutex::new(()),
        })
    }

    const fn table_definition(name: &str) -> RedbTableDefinition<'_> {
        TableDefinition::new(name)
    }

    fn with_read_table<R>(
        &self,
        operation: impl FnOnce(
            &redb::ReadOnlyTable<&'static [u8], &'static [u8]>,
        ) -> Result<R, RepositoryError>,
    ) -> Result<R, RepositoryError> {
        let read_txn = self
            .db
            .begin_read()
            .map_err(|error| RepositoryError::Query(error.to_string()))?;
        let table = read_txn
            .open_table(Self::table_definition(&self.name))
            .map_err(|error| RepositoryError::Query(error.to_string()))?;
        operation(&table)
    }

    fn with_write_table<R>(
        &self,
        error_mapper: fn(String) -> RepositoryError,
        operation: impl FnOnce(
            &mut redb::Table<'_, &'static [u8], &'static [u8]>,
        ) -> Result<R, RepositoryError>,
    ) -> Result<R, RepositoryError> {
        let _guard = self
            .write_lock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let write_txn = self
            .db
            .begin_write()
            .map_err(|error| error_mapper(error.to_string()))?;
        let result = {
            let mut table = write_txn
                .open_table(Self::table_definition(&self.name))
                .map_err(|error| error_mapper(error.to_string()))?;
            operation(&mut table)?
        };
        write_txn
            .commit()
            .map_err(|error| error_mapper(error.to_string()))?;
        Ok(result)
    }
}

impl KvTree for RedbTree {
    fn insert(&self, key: &[u8], value: &[u8]) -> Result<(), RepositoryError> {
        self.with_write_table(RepositoryError::Insert, |table| {
            let replaced = table
                .insert(key, value)
                .map_err(|error| RepositoryError::Insert(error.to_string()))?;
            Ok(replaced.is_some())
        })?;
        Ok(())
    }

    fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, RepositoryError> {
        self.with_read_table(|table| {
            let value = table
                .get(key)
                .map_err(|error| RepositoryError::Query(error.to_string()))?;
            Ok(value.map(|guard| guard.value().to_vec()))
        })
    }

    fn remove(&self, key: &[u8]) -> Result<bool, RepositoryError> {
        let removed = self.with_write_table(RepositoryError::Delete, |table| {
            let removed = table
                .remove(key)
                .map_err(|error| RepositoryError::Delete(error.to_string()))?;
            Ok(removed.is_some())
        })?;

        Ok(removed)
    }

    fn len(&self) -> usize {
        self.with_read_table(|table| {
            table
                .len()
                .map(|len| usize::try_from(len).unwrap_or(usize::MAX))
                .map_err(|error| RepositoryError::Query(error.to_string()))
        })
        .unwrap_or_else(|error| {
            tracing::warn!(error = %error, tree = %self.name, "failed to read tree length");
            0
        })
    }

    #[cfg(test)]
    fn clear(&self) -> Result<(), RepositoryError> {
        self.with_write_table(RepositoryError::Delete, |table| {
            table
                .retain(|_, _| false)
                .map_err(|error| RepositoryError::Delete(error.to_string()))?;
            Ok(())
        })?;
        Ok(())
    }

    fn scan_ascending(
        &self,
        callback: &mut dyn FnMut(&[u8], &[u8]) -> bool,
    ) -> Result<(), RepositoryError> {
        self.with_read_table(|table| {
            let iter = table
                .iter()
                .map_err(|error| RepositoryError::Query(error.to_string()))?;

            for entry in iter {
                let (key, value) =
                    entry.map_err(|error| RepositoryError::Query(error.to_string()))?;
                if !callback(key.value(), value.value()) {
                    break;
                }
            }
            Ok(())
        })
    }

    fn scan_descending(
        &self,
        callback: &mut dyn FnMut(&[u8], &[u8]) -> bool,
    ) -> Result<(), RepositoryError> {
        self.with_read_table(|table| {
            let iter = table
                .iter()
                .map_err(|error| RepositoryError::Query(error.to_string()))?;

            for entry in iter.rev() {
                let (key, value) =
                    entry.map_err(|error| RepositoryError::Query(error.to_string()))?;
                if !callback(key.value(), value.value()) {
                    break;
                }
            }
            Ok(())
        })
    }
}

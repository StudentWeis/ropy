#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Shared test helpers for repository tests.

use std::{thread, time::Duration};

use tempfile::tempdir;

use super::{
    backend::{BackendFactory, StorageBackend},
    memory_backend::{MemoryBackend, memory_backend_factory},
    models::ClipboardRecord,
    repo::ClipboardRepository,
};

type MemoryClipboardRepository = ClipboardRepository<MemoryBackend>;

pub fn create_test_repo_with<B: StorageBackend>(
    factory: BackendFactory<B>,
) -> (tempfile::TempDir, ClipboardRepository<B>) {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let repo = ClipboardRepository::init(&db_path, temp_dir.path().join("images"), factory)
        .expect("Failed to create test repository");
    (temp_dir, repo)
}

pub fn create_test_repo() -> MemoryClipboardRepository {
    let (_temp_dir, repo) = create_test_repo_with(memory_backend_factory);
    repo
}

pub fn load_display_records<B: StorageBackend>(
    repo: &ClipboardRepository<B>,
    limit: usize,
) -> Vec<ClipboardRecord> {
    repo.get_display_records(limit)
        .expect("Failed to get display records")
}

pub fn save_numbered_records<B: StorageBackend>(repo: &ClipboardRepository<B>, count: usize) {
    for i in 1..=count {
        repo.save_text(format!("Record {i}"))
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
    }
}

#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Shared test helpers for repository tests.

use std::{thread, time::Duration};

use tempfile::tempdir;

use super::{
    backend::BackendFactory, memory_backend::memory_backend_factory, models::ClipboardRecord,
    repo::ClipboardRepository,
};

pub fn create_test_repo_with(factory: BackendFactory) -> (tempfile::TempDir, ClipboardRepository) {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let repo = ClipboardRepository::init(&db_path, temp_dir.path().join("images"), factory)
        .expect("Failed to create test repository");
    (temp_dir, repo)
}

pub fn create_test_repo() -> ClipboardRepository {
    let (_temp_dir, repo) = create_test_repo_with(memory_backend_factory);
    repo
}

pub fn load_display_records(repo: &ClipboardRepository, limit: usize) -> Vec<ClipboardRecord> {
    repo.get_display_records(limit)
        .expect("Failed to get display records")
}

pub fn save_numbered_records(repo: &ClipboardRepository, count: usize) {
    for i in 1..=count {
        repo.save_text(format!("Record {i}"))
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
    }
}

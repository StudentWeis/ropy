#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Shared test helpers for repository tests.

use chrono::{DateTime, Local};
use tempfile::tempdir;

use super::{
    backend::{
        BackendFactory, StorageBackend,
        memory::{MemoryBackend, memory_backend_factory},
    },
    models::{ClipboardRecord, ContentType},
    repo::ClipboardRepository,
};

type MemoryClipboardRepository = ClipboardRepository<MemoryBackend>;

pub(super) fn create_test_repo_with<B: StorageBackend>(
    factory: BackendFactory<B>,
) -> (tempfile::TempDir, ClipboardRepository<B>) {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");
    let repo = ClipboardRepository::init(&db_path, temp_dir.path().join("images"), factory)
        .expect("Failed to create test repository");
    (temp_dir, repo)
}

pub(super) fn create_test_repo() -> (tempfile::TempDir, MemoryClipboardRepository) {
    create_test_repo_with(memory_backend_factory)
}

#[test]
fn test_memory_fixture_keeps_sidecars_until_teardown() {
    let (temp_dir, repo) = create_test_repo();
    let root = temp_dir.path().to_path_buf();
    assert!(root.is_dir());
    let record = repo
        .save_rich_text("fixture".into(), Some("<b>fixture</b>"), None)
        .unwrap();
    assert_eq!(
        super::assets::load_rich_text_html(record.rich_text_meta.as_ref().unwrap()).unwrap(),
        "<b>fixture</b>"
    );
    drop(repo);
    drop(temp_dir);
    assert!(!root.exists());
}

pub(super) fn load_display_records<B: StorageBackend>(
    repo: &ClipboardRepository<B>,
    limit: usize,
) -> Vec<ClipboardRecord> {
    repo.get_display_records(limit)
        .expect("Failed to get display records")
}

pub(super) fn capture_time(seconds: i64) -> DateTime<Local> {
    DateTime::from_timestamp(1_700_000_000 + seconds, 0)
        .expect("valid fixture timestamp")
        .with_timezone(&Local)
}

pub(super) fn save_text_at<B: StorageBackend>(
    repo: &ClipboardRepository<B>,
    content: impl Into<String>,
    seconds: i64,
) -> ClipboardRecord {
    repo.save_at(content.into(), ContentType::Text, capture_time(seconds))
        .expect("Failed to save fixture text")
}

pub(super) fn save_numbered_records<B: StorageBackend>(
    repo: &ClipboardRepository<B>,
    count: usize,
) {
    for i in 1..=count {
        save_text_at(
            repo,
            format!("Record {i}"),
            i64::try_from(i).expect("fixture count fits i64"),
        );
    }
}

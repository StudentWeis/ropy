#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Tests for `save*` paths: text, files, images, rich-text, and serialization
//! round-trips.

use std::{thread, time::Duration};

use crate::repository::{
    backend::{BackendFactory, StorageBackend},
    memory_backend::memory_backend_factory,
    models::{ClipboardRecord, ContentType},
    redb_backend::redb_backend_factory,
    test_helpers::{create_test_repo, create_test_repo_with},
};

fn assert_save_and_get_text_with<B: StorageBackend>(factory: BackendFactory<B>) {
    let (_temp_dir, repo) = create_test_repo_with(factory);

    let record = repo
        .save_text("Hello, World!".to_string())
        .unwrap_or_else(|error| panic!("Failed to save: {error}"));
    assert_eq!(record.content, "Hello, World!");
    assert_eq!(record.content_type, ContentType::Text);

    let retrieved = repo
        .get_by_id(record.id)
        .unwrap_or_else(|error| panic!("Failed to get by id: {error}"))
        .unwrap_or_else(|| panic!("Record not found"));
    assert_eq!(retrieved.content, "Hello, World!");
}

#[test]
fn test_save_and_get_text_redb() {
    assert_save_and_get_text_with(redb_backend_factory);
}

#[test]
fn test_save_and_get_text_memory() {
    assert_save_and_get_text_with(memory_backend_factory);
}

#[test]
fn test_save_rich_text_persists_meta_and_sidecar_files() {
    let (temp_dir, repo) = create_test_repo_with(memory_backend_factory);

    let record = repo
        .save_rich_text(
            "hello".to_string(),
            Some("<p>hello</p>"),
            Some("{\\rtf1 hello}"),
        )
        .expect("Failed to save rich text");

    assert_eq!(record.content_type, ContentType::RichText);

    let meta = record
        .rich_text_meta
        .as_ref()
        .expect("Rich text metadata should be present");
    let html_path = meta
        .html_path
        .as_ref()
        .expect("HTML path should be present");
    let rtf_path = meta.rtf_path.as_ref().expect("RTF path should be present");

    assert!(temp_dir.path().join("rich_text").exists());
    assert_eq!(
        std::fs::read_to_string(html_path).expect("Failed to read html"),
        "<p>hello</p>"
    );
    assert_eq!(
        std::fs::read_to_string(rtf_path).expect("Failed to read rtf"),
        "{\\rtf1 hello}"
    );
}

#[test]
fn test_save_empty_content() {
    let repo = create_test_repo();

    let record = repo.save_text(String::new()).expect("Failed to save");
    assert_eq!(record.content, "");

    let retrieved = repo
        .get_by_id(record.id)
        .expect("Failed to get")
        .expect("Not found");
    assert_eq!(retrieved.content, "");
}

#[test]
fn test_save_very_long_content() {
    let repo = create_test_repo();

    let long_content = "x".repeat(100_000);
    let record = repo
        .save_text(long_content.clone())
        .expect("Failed to save");

    let retrieved = repo
        .get_by_id(record.id)
        .expect("Failed to get")
        .expect("Not found");
    assert_eq!(retrieved.content.len(), 100_000);
    assert_eq!(retrieved.content, long_content);
}

#[test]
fn test_save_unicode_content() {
    let repo = create_test_repo();

    let contents = vec![
        "Hello 世界 🌍",
        "مرحبا بالعالم",
        "🎉🎊🎁",
        "日本語テキスト",
        "Special chars: <>&\"'",
    ];

    for content in contents {
        let record = repo.save_text(content.to_string()).expect("Failed to save");
        let retrieved = repo
            .get_by_id(record.id)
            .expect("Failed to get")
            .expect("Not found");
        assert_eq!(retrieved.content, content);
    }
}

#[test]
fn test_save_different_content_types() {
    let repo = create_test_repo();

    let text_record = repo
        .save("text content".to_string(), ContentType::Text)
        .expect("Failed to save text");
    assert_eq!(text_record.content_type, ContentType::Text);

    let path_record = repo
        .save("/path/to/file".to_string(), ContentType::FilePath)
        .expect("Failed to save path");
    assert_eq!(path_record.content_type, ContentType::FilePath);

    assert_eq!(repo.count(), 2);
}

#[test]
fn test_repository_save_files_normalizes_and_persists_file_payload() {
    let repo = create_test_repo();

    let record = repo
        .save_files(&[
            "file:///tmp/alpha%20file.txt".to_string(),
            "/tmp/beta.txt".to_string(),
        ])
        .expect("Failed to save file paths");

    assert_eq!(record.content_type, ContentType::FilePath);
    assert_eq!(
        record.content,
        "[\"/tmp/alpha file.txt\",\"/tmp/beta.txt\"]"
    );
}

#[test]
fn test_repository_save_files_when_equivalent_inputs_reuses_existing_record() {
    let repo = create_test_repo();

    let first = repo
        .save_files(&["file:///tmp/demo%20file.txt".to_string()])
        .expect("Failed to save first file record");
    let second = repo
        .save_files(&["/tmp/demo file.txt".to_string()])
        .expect("Failed to save second file record");

    assert_eq!(first.id, second.id);
    assert_eq!(repo.count(), 1);
}

#[test]
fn test_save_image_from_path_basic() {
    let repo = create_test_repo();

    let path = "/tmp/test_image.png".to_string();
    let hash = 12345_u64;

    let record = repo
        .save_image_from_path(path.clone(), hash)
        .expect("Failed to save image");

    assert_eq!(record.content, path);
    assert_eq!(record.content_type, ContentType::Image);
    assert_eq!(record.id, hash);
}

#[test]
fn test_save_image_from_path_dedup() {
    let repo = create_test_repo();

    let path1 = "/tmp/image1.png".to_string();
    let path2 = "/tmp/image2.png".to_string();
    let hash = 12345_u64;

    let r1 = repo
        .save_image_from_path(path1, hash)
        .expect("Failed to save first");

    thread::sleep(Duration::from_millis(10));

    let r2 = repo
        .save_image_from_path(path2, hash)
        .expect("Failed to save second");

    assert_eq!(r1.id, r2.id);
    assert_eq!(repo.count(), 1);
    assert!(r2.created_at > r1.created_at);
}

#[test]
fn test_save_after_clear() {
    let repo = create_test_repo();

    repo.save_text("Before clear".to_string())
        .expect("Failed to save");
    repo.clear().expect("Failed to clear");

    let record = repo
        .save_text("After clear".to_string())
        .expect("Failed to save after clear");
    assert_eq!(repo.count(), 1);
    assert_eq!(record.content, "After clear");
}

#[test]
fn test_binary_serialization_round_trip() {
    use crate::repository::{backend::KvTree, test_helpers::load_display_records};

    let repo = create_test_repo();
    let now = chrono::Local::now();
    let record = ClipboardRecord {
        id: 1000_u64,
        content: "binary record".to_string(),
        created_at: now,
        content_type: ContentType::Text,
        pinned: false,
        rich_text_meta: None,
    };
    let key = 1000_u64.to_be_bytes();
    let value = postcard::to_allocvec(&record).expect("failed to serialize");
    repo.records.insert(&key, &value).expect("failed to insert");

    repo.time_index
        .insert_raw(now.timestamp_millis(), 1000, false, &ContentType::Text);

    let records = load_display_records(&repo, 10);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].content, "binary record");
    assert!(!records[0].pinned);
}

#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Tests for display ordering, lookup, and multi-step sequences.

use std::thread;

use crate::repository::{
    models::{ClipboardRecord, ContentType},
    repo::ClipboardRepository,
    test_helpers::{
        capture_time, create_test_repo, load_display_records, save_numbered_records, save_text_at,
    },
};

#[rstest::rstest]
#[case(false)]
#[case(true)]
fn test_same_millisecond_limit_and_cleanup_keep_newest(#[case] cleanup: bool) {
    use crate::repository::backend::{KvTree, RECORDS_TREE, StorageBackend, memory::MemoryBackend};

    let directory = tempfile::tempdir().unwrap();
    let backend = MemoryBackend::new();
    let images = directory.path().join("images");
    drop(ClipboardRepository::from_backend(backend.clone(), images.clone()).unwrap());
    let records = backend.open_tree(RECORDS_TREE).unwrap();
    // Reverse ID order deliberately: hashes must not decide capture chronology.
    for (id, nanos) in [(10_u64, 100), (1, 200)] {
        let captured_at = chrono::DateTime::from_timestamp(1_790_000_000, nanos)
            .unwrap()
            .with_timezone(&chrono::Local);
        let record = ClipboardRecord::new(id, id.to_string(), captured_at, ContentType::Text);
        records
            .insert(&id.to_be_bytes(), &postcard::to_allocvec(&record).unwrap())
            .unwrap();
    }
    let repo = ClipboardRepository::from_backend(backend, images).unwrap();
    if cleanup {
        assert_eq!(repo.cleanup_old_records(1).unwrap(), 1);
        assert_eq!(repo.count(), 1);
        assert!(repo.get_by_id(10).unwrap().is_none());
    }
    let selected = repo.get_display_records(1).unwrap();
    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].id, 1);
}

#[rstest::rstest]
#[case(false)]
#[case(true)]
fn test_display_and_cleanup_across_epoch_keep_newest(#[case] cleanup: bool) {
    let (_directory, repo) = create_test_repo();
    let older = repo
        .save_at(
            "before epoch".into(),
            ContentType::Text,
            "1969-12-31T23:59:59+00:00".parse().unwrap(),
        )
        .unwrap();
    let newer = repo
        .save_at(
            "modern".into(),
            ContentType::Text,
            "2023-11-14T22:13:20+00:00".parse().unwrap(),
        )
        .unwrap();
    if cleanup {
        assert_eq!(repo.cleanup_old_records(1).unwrap(), 1);
        assert!(repo.get_by_id(older.id).unwrap().is_none());
        assert!(repo.get_by_id(newer.id).unwrap().is_some());
    }
    assert_eq!(repo.get_display_records(1).unwrap()[0].id, newer.id);
}

#[test]
fn test_late_rich_text_capture_preserves_newer_payload() {
    let (_temp_dir, repo) = create_test_repo();
    let newer = capture_time(2);
    let older = newer - chrono::Duration::seconds(1);
    let original = repo
        .save_rich_text_at("same".into(), Some("<b>new</b>"), None, newer)
        .unwrap();
    repo.save_rich_text_at("same".into(), Some("<i>old</i>"), None, older)
        .unwrap();
    let stored = repo.get_by_id(original.id).unwrap().unwrap();
    assert_eq!(stored.created_at, newer);
    assert_eq!(stored.rich_text_meta, original.rich_text_meta);
    assert_eq!(
        crate::repository::assets::load_rich_text_html(stored.rich_text_meta.as_ref().unwrap())
            .unwrap(),
        "<b>new</b>"
    );
}

#[test]
fn test_get_display_records() {
    let (_temp_dir, repo) = create_test_repo();

    save_numbered_records(&repo, 5);

    let recent = load_display_records(&repo, 3);
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].content, "Record 5");
    assert_eq!(recent[1].content, "Record 4");
    assert_eq!(recent[2].content, "Record 3");
}

#[test]
fn test_get_display_records_when_favorites_present_keep_default_time_order() {
    let (_temp_dir, repo) = create_test_repo();

    let favorite_and_pinned = save_text_at(&repo, "Favorite pinned", 1);
    let favorite_only = save_text_at(&repo, "Favorite only", 2);
    save_text_at(&repo, "Ordinary old", 3);
    save_text_at(&repo, "Ordinary new", 4);
    save_text_at(&repo, "Ordinary newest", 5);

    repo.toggle_favorite(favorite_and_pinned.id)
        .expect("Failed to favorite pinned record");
    repo.toggle_pin(favorite_and_pinned.id)
        .expect("Failed to pin favorite record");
    repo.toggle_favorite(favorite_only.id)
        .expect("Failed to favorite record");

    let display_records = repo
        .get_display_records(2)
        .expect("Failed to get display records");
    let contents = display_records
        .iter()
        .map(|record| record.content.as_str())
        .collect::<Vec<_>>();

    assert_eq!(
        contents,
        vec![
            "Favorite pinned",
            "Ordinary newest",
            "Ordinary new",
            "Favorite only",
        ]
    );
}

#[test]
fn test_get_display_records_when_total_is_41_with_two_favorites_and_one_pinned_returns_41() {
    let (_temp_dir, repo) = create_test_repo();

    let first = save_text_at(&repo, "Record 1", 1);
    let second = save_text_at(&repo, "Record 2", 2);
    let third = save_text_at(&repo, "Record 3", 3);

    repo.toggle_favorite(first.id)
        .expect("Failed to favorite first");
    repo.toggle_favorite(second.id)
        .expect("Failed to favorite second");
    repo.toggle_pin(third.id).expect("Failed to pin third");

    for index in 4..=41 {
        save_text_at(&repo, format!("Record {index}"), index);
    }

    let display_records = repo
        .get_display_records(40)
        .expect("Failed to get display records");

    assert_eq!(repo.count(), 41);
    assert_eq!(display_records.len(), 41);
}

#[test]
fn test_get_display_records_when_limit_is_40_with_two_favorites_and_one_pinned_returns_43() {
    let (_temp_dir, repo) = create_test_repo();

    let first = save_text_at(&repo, "Record 1", 1);
    let second = save_text_at(&repo, "Record 2", 2);
    let third = save_text_at(&repo, "Record 3", 3);

    repo.toggle_favorite(first.id)
        .expect("Failed to favorite first");
    repo.toggle_favorite(second.id)
        .expect("Failed to favorite second");
    repo.toggle_pin(third.id).expect("Failed to pin third");

    for index in 4..=43 {
        save_text_at(&repo, format!("Record {index}"), index);
    }

    let display_records = repo
        .get_display_records(40)
        .expect("Failed to get display records");

    assert_eq!(repo.count(), 43);
    assert_eq!(display_records.len(), 43);
}

#[test]
fn test_get_display_records_when_newest_records_are_favorited_still_returns_43() {
    let (_temp_dir, repo) = create_test_repo();

    let pinned = save_text_at(&repo, "Pinned", 1);

    for index in 2..=43 {
        save_text_at(&repo, format!("Record {index}"), index);
    }

    repo.toggle_pin(pinned.id).expect("Failed to pin");

    let newest = repo.get_by_id(crate::repository::content_hash(
        "Record 43",
        &ContentType::Text,
    ));
    let second_newest = repo.get_by_id(crate::repository::content_hash(
        "Record 42",
        &ContentType::Text,
    ));

    let newest = newest
        .expect("Failed to get newest")
        .expect("Newest should exist");
    let second_newest = second_newest
        .expect("Failed to get second newest")
        .expect("Second newest should exist");

    repo.toggle_favorite(newest.id)
        .expect("Failed to favorite newest");
    repo.toggle_favorite(second_newest.id)
        .expect("Failed to favorite second newest");

    let display_records = repo
        .get_display_records(40)
        .expect("Failed to get display records");

    assert_eq!(repo.count(), 43);
    assert_eq!(display_records.len(), 43);
}

#[test]
fn test_get_display_records_zero_limit() {
    let (_temp_dir, repo) = create_test_repo();

    repo.save_text("Test".to_string()).expect("Failed to save");

    let result = load_display_records(&repo, 0);
    assert_eq!(result, []);
}

#[test]
fn test_get_display_records_large_limit() {
    let (_temp_dir, repo) = create_test_repo();

    save_numbered_records(&repo, 3);

    let result = load_display_records(&repo, 1000);
    assert_eq!(result.len(), 3);
}

#[test]
fn test_get_display_records_empty_repo() {
    let (_temp_dir, repo) = create_test_repo();

    let result = load_display_records(&repo, 10);
    assert_eq!(result, []);
}

#[test]
fn test_get_display_records_with_pinned_and_limit() {
    let (_temp_dir, repo) = create_test_repo();

    for i in 1..=5 {
        save_text_at(&repo, format!("Unpinned {i}"), i);
    }

    let recent = load_display_records(&repo, 5);
    let oldest_id = recent[4].id;
    repo.toggle_pin(oldest_id).expect("Failed to pin");

    let recent_limited = load_display_records(&repo, 3);

    assert_eq!(recent_limited.len(), 4);
    assert!(recent_limited[0].pinned);
    assert_eq!(recent_limited[0].id, oldest_id);
    assert_eq!(recent_limited[1].content, "Unpinned 5");
    assert_eq!(recent_limited[2].content, "Unpinned 4");
    assert_eq!(recent_limited[3].content, "Unpinned 3");
}

#[test]
fn test_get_by_id_nonexistent() {
    let (_temp_dir, repo) = create_test_repo();

    let result = repo.get_by_id(999_999_999).expect("Failed to get by id");
    assert!(result.is_none());
}

#[test]
fn test_get_by_id_zero() {
    let (_temp_dir, repo) = create_test_repo();

    let result = repo.get_by_id(0).expect("Failed to get by id");
    assert!(result.is_none());
}

#[test]
fn test_get_by_id_u64_max() {
    let (_temp_dir, repo) = create_test_repo();

    let result = repo.get_by_id(u64::MAX).expect("Failed to get by id");
    assert!(result.is_none());
}

#[test]
fn test_count_empty() {
    let (_temp_dir, repo) = create_test_repo();

    assert_eq!(repo.count(), 0);
}

#[test]
fn test_sort_for_display_empty() {
    let mut records: Vec<ClipboardRecord> = vec![];
    ClipboardRepository::sort_for_display(&mut records);
    assert_eq!(records, []);
}

#[test]
fn test_sort_for_display_single() {
    let now = capture_time(0);
    let mut records = vec![ClipboardRecord {
        id: 1,
        content: "only".to_string(),
        created_at: now,
        content_type: ContentType::Text,
        pinned: false,
        rich_text_meta: None,
    }];

    ClipboardRepository::sort_for_display(&mut records);
    assert_eq!(records.len(), 1);
    assert!(!records[0].pinned);
}

#[test]
fn test_sort_for_display_all_same_pinned_state() {
    let now = capture_time(0);
    let mut records = vec![
        ClipboardRecord {
            id: 1,
            content: "oldest".to_string(),
            created_at: now - chrono::Duration::seconds(2),
            content_type: ContentType::Text,
            pinned: false,
            rich_text_meta: None,
        },
        ClipboardRecord {
            id: 2,
            content: "middle".to_string(),
            created_at: now - chrono::Duration::seconds(1),
            content_type: ContentType::Text,
            pinned: false,
            rich_text_meta: None,
        },
        ClipboardRecord {
            id: 3,
            content: "newest".to_string(),
            created_at: now,
            content_type: ContentType::Text,
            pinned: false,
            rich_text_meta: None,
        },
    ];

    ClipboardRepository::sort_for_display(&mut records);

    assert_eq!(records[0].content, "newest");
    assert_eq!(records[1].content, "middle");
    assert_eq!(records[2].content, "oldest");
}

#[test]
fn test_multiple_operations_sequence() {
    let (_temp_dir, repo) = create_test_repo();

    let r1 = save_text_at(&repo, "First", 1);
    let r2 = save_text_at(&repo, "Second", 2);
    let _r3 = save_text_at(&repo, "Third", 3);

    assert_eq!(repo.count(), 3);

    repo.toggle_pin(r1.id).expect("Failed to pin");

    let deleted = repo.delete(r2.id).expect("Failed to delete");
    assert!(deleted);
    assert_eq!(repo.count(), 2);

    let recent = load_display_records(&repo, 10);
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].content, "First");
    assert_eq!(recent[1].content, "Third");

    repo.clear().expect("Failed to clear");
    assert_eq!(repo.count(), 0);
}

#[test]
fn test_concurrent_saves_then_deletes_remove_selected_records() {
    let (_temp_dir, repo) = create_test_repo();
    let repo = std::sync::Arc::new(repo);
    let mut handles = vec![];

    for i in 0..10 {
        let r = repo.clone();
        let handle =
            thread::spawn(move || r.save_text(format!("thread {i}")).expect("Failed to save"));
        handles.push(handle);
    }

    let mut ids = vec![];
    for handle in handles {
        let record = handle.join().expect("Thread panicked");
        ids.push(record.id);
    }

    assert_eq!(repo.count(), 10);

    let repo2 = repo.clone();
    let ids_to_delete = ids[..5].to_vec();
    let handle = thread::spawn(move || {
        for id in ids_to_delete {
            repo2.delete(id).expect("Failed to delete");
        }
    });

    handle.join().expect("Delete thread panicked");
    assert_eq!(repo.count(), 5);
}

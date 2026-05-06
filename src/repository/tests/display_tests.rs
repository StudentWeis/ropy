#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Tests for display ordering, lookup, and multi-step sequences.

use std::{thread, time::Duration};

use crate::repository::{
    models::{ClipboardRecord, ContentType},
    repo::ClipboardRepository,
    test_helpers::{create_test_repo, load_display_records},
};

#[test]
fn test_get_display_records() {
    let repo = create_test_repo();

    for i in 1..=5 {
        repo.save_text(format!("Record {i}"))
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
    }

    let recent = load_display_records(&repo, 3);
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].content, "Record 5");
    assert_eq!(recent[1].content, "Record 4");
    assert_eq!(recent[2].content, "Record 3");
}

#[test]
fn test_get_display_records_when_favorites_present_keep_default_time_order() {
    let repo = create_test_repo();

    let favorite_and_pinned = repo
        .save_text("Favorite pinned".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let favorite_only = repo
        .save_text("Favorite only".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    repo.save_text("Ordinary old".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    repo.save_text("Ordinary new".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    repo.save_text("Ordinary newest".to_string())
        .expect("Failed to save");

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
    let repo = create_test_repo();

    let first = repo
        .save_text("Record 1".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let second = repo
        .save_text("Record 2".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let third = repo
        .save_text("Record 3".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));

    repo.toggle_favorite(first.id)
        .expect("Failed to favorite first");
    repo.toggle_favorite(second.id)
        .expect("Failed to favorite second");
    repo.toggle_pin(third.id).expect("Failed to pin third");

    for index in 4..=41 {
        repo.save_text(format!("Record {index}"))
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
    }

    let display_records = repo
        .get_display_records(40)
        .expect("Failed to get display records");

    assert_eq!(repo.count(), 41);
    assert_eq!(display_records.len(), 41);
}

#[test]
fn test_get_display_records_when_limit_is_40_with_two_favorites_and_one_pinned_returns_43() {
    let repo = create_test_repo();

    let first = repo
        .save_text("Record 1".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let second = repo
        .save_text("Record 2".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let third = repo
        .save_text("Record 3".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));

    repo.toggle_favorite(first.id)
        .expect("Failed to favorite first");
    repo.toggle_favorite(second.id)
        .expect("Failed to favorite second");
    repo.toggle_pin(third.id).expect("Failed to pin third");

    for index in 4..=43 {
        repo.save_text(format!("Record {index}"))
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
    }

    let display_records = repo
        .get_display_records(40)
        .expect("Failed to get display records");

    assert_eq!(repo.count(), 43);
    assert_eq!(display_records.len(), 43);
}

#[test]
fn test_get_display_records_when_newest_records_are_favorited_still_returns_43() {
    let repo = create_test_repo();

    let pinned = repo
        .save_text("Pinned".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));

    for index in 2..=43 {
        repo.save_text(format!("Record {index}"))
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
    }

    repo.toggle_pin(pinned.id).expect("Failed to pin");

    let newest = repo.get_by_id(crate::utils::content_hash("Record 43", &ContentType::Text));
    let second_newest = repo.get_by_id(crate::utils::content_hash("Record 42", &ContentType::Text));

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
    let repo = create_test_repo();

    repo.save_text("Test".to_string()).expect("Failed to save");

    let result = load_display_records(&repo, 0);
    assert!(result.is_empty());
}

#[test]
fn test_get_display_records_large_limit() {
    let repo = create_test_repo();

    for i in 1..=3 {
        repo.save_text(format!("Record {i}"))
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
    }

    let result = load_display_records(&repo, 1000);
    assert_eq!(result.len(), 3);
}

#[test]
fn test_get_display_records_empty_repo() {
    let repo = create_test_repo();

    let result = load_display_records(&repo, 10);
    assert!(result.is_empty());
}

#[test]
fn test_get_display_records_with_pinned_and_limit() {
    let repo = create_test_repo();

    for i in 1..=5 {
        repo.save_text(format!("Unpinned {i}"))
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
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
    let repo = create_test_repo();

    let result = repo.get_by_id(999_999_999).expect("Failed to get by id");
    assert!(result.is_none());
}

#[test]
fn test_get_by_id_zero() {
    let repo = create_test_repo();

    let result = repo.get_by_id(0).expect("Failed to get by id");
    assert!(result.is_none());
}

#[test]
fn test_get_by_id_u64_max() {
    let repo = create_test_repo();

    let result = repo.get_by_id(u64::MAX).expect("Failed to get by id");
    assert!(result.is_none());
}

#[test]
fn test_count_empty() {
    let repo = create_test_repo();

    assert_eq!(repo.count(), 0);
}

#[test]
fn test_sort_for_display_empty() {
    let mut records: Vec<ClipboardRecord> = vec![];
    ClipboardRepository::sort_for_display(&mut records);
    assert!(records.is_empty());
}

#[test]
fn test_sort_for_display_single() {
    let now = chrono::Local::now();
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
    let now = chrono::Local::now();
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
    let repo = create_test_repo();

    let r1 = repo.save_text("First".to_string()).expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let r2 = repo
        .save_text("Second".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let _r3 = repo.save_text("Third".to_string()).expect("Failed to save");

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
fn test_concurrent_save_and_delete() {
    let repo = create_test_repo();
    let repo = std::sync::Arc::new(repo);
    let mut handles = vec![];

    for i in 0..10 {
        let r = repo.clone();
        let handle =
            std::thread::spawn(move || r.save_text(format!("thread {i}")).expect("Failed to save"));
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
    let handle = std::thread::spawn(move || {
        for id in ids_to_delete {
            repo2.delete(id).expect("Failed to delete");
        }
    });

    handle.join().expect("Delete thread panicked");
    assert_eq!(repo.count(), 5);
}

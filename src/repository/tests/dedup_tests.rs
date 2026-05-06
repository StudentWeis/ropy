#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Tests for deduplication, cleanup, favorites, delete, and clear.

use std::{thread, time::Duration};

use rstest::rstest;

use crate::repository::{
    backend::{BackendFactory, StorageBackend},
    memory_backend::memory_backend_factory,
    redb_backend::redb_backend_factory,
    repo::ClipboardRepository,
    test_helpers::{
        create_test_repo, create_test_repo_with, load_display_records, save_numbered_records,
    },
};

#[test]
fn test_dedup_same_content() {
    let repo = create_test_repo();

    let r1 = repo
        .save_text("duplicate".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let r2 = repo
        .save_text("duplicate".to_string())
        .expect("Failed to save");

    assert_eq!(r1.id, r2.id);
    assert_eq!(repo.count(), 1);
    assert!(r2.created_at > r1.created_at);
}

#[test]
fn test_dedup_aba_pattern() {
    let repo = create_test_repo();

    repo.save_text("A".to_string()).expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    repo.save_text("B".to_string()).expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let a2 = repo.save_text("A".to_string()).expect("Failed to save");

    assert_eq!(repo.count(), 2);
    let recent = load_display_records(&repo, 2);
    assert_eq!(recent[0].content, "A");
    assert_eq!(recent[0].created_at, a2.created_at);
    assert_eq!(recent[1].content, "B");
}

#[test]
fn test_content_hash_deterministic() {
    let repo = create_test_repo();

    let r1 = repo
        .save_text("stable hash".to_string())
        .expect("Failed to save");
    let expected_id = r1.id;

    let r2 = repo
        .save_text("stable hash".to_string())
        .expect("Failed to save");
    assert_eq!(r2.id, expected_id);
}

fn assert_cleanup_old_records_with<B: StorageBackend>(factory: BackendFactory<B>) {
    let (_temp_dir, repo) = create_test_repo_with(factory);

    for i in 1..=10 {
        repo.save_text(format!("Record {i}"))
            .unwrap_or_else(|error| panic!("Failed to save: {error}"));
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(repo.count(), 10);

    let removed = repo
        .cleanup_old_records(5)
        .unwrap_or_else(|error| panic!("Failed to clean up: {error}"));
    assert_eq!(removed, 5);
    assert_eq!(repo.count(), 5);

    let recent = load_display_records(&repo, 5);
    assert_eq!(recent[0].content, "Record 10");
    assert_eq!(recent[4].content, "Record 6");
}

#[test]
fn test_cleanup_old_records_redb() {
    assert_cleanup_old_records_with(redb_backend_factory);
}

#[test]
fn test_cleanup_old_records_memory() {
    assert_cleanup_old_records_with(memory_backend_factory);
}

#[test]
fn test_cleanup_old_records_zero_keep() {
    let repo = create_test_repo();

    repo.save_text("Test".to_string()).expect("Failed to save");

    let removed = repo.cleanup_old_records(0).expect("Failed to clean up");
    assert_eq!(removed, 1);
    assert_eq!(repo.count(), 0);
}

#[test]
fn test_cleanup_old_records_greater_than_total() {
    let repo = create_test_repo();

    repo.save_text("Test".to_string()).expect("Failed to save");

    let removed = repo.cleanup_old_records(100).expect("Failed to clean up");
    assert_eq!(removed, 0);
    assert_eq!(repo.count(), 1);
}

#[test]
fn test_cleanup_old_records_equal_to_total() {
    let repo = create_test_repo();

    repo.save_text("Test".to_string()).expect("Failed to save");

    let removed = repo.cleanup_old_records(1).expect("Failed to clean up");
    assert_eq!(removed, 0);
    assert_eq!(repo.count(), 1);
}

#[rstest]
#[case(10)]
#[case(50)]
fn test_cleanup_old_records_if_needed_at_threshold_preserves_records(#[case] keep_count: usize) {
    let repo = create_test_repo();
    let threshold = keep_count + ClipboardRepository::cleanup_buffer_record_count(keep_count);

    save_numbered_records(&repo, threshold);

    let removed = repo
        .cleanup_old_records_if_needed(keep_count)
        .unwrap_or_else(|err| panic!("Failed to conditionally clean up: {err}"));

    assert_eq!(removed, 0);
    assert_eq!(repo.count(), threshold);
}

#[rstest]
#[case(10)]
#[case(50)]
fn test_cleanup_old_records_if_needed_above_threshold_trims_to_keep_count(
    #[case] keep_count: usize,
) {
    let repo = create_test_repo();
    let threshold = keep_count + ClipboardRepository::cleanup_buffer_record_count(keep_count);

    save_numbered_records(&repo, threshold + 1);

    let removed = repo
        .cleanup_old_records_if_needed(keep_count)
        .unwrap_or_else(|err| panic!("Failed to conditionally clean up: {err}"));

    assert_eq!(removed, threshold + 1 - keep_count);
    assert_eq!(repo.count(), keep_count);

    let recent = repo
        .get_display_records(keep_count)
        .unwrap_or_else(|err| panic!("Failed to get display records: {err}"));
    assert_eq!(recent.len(), keep_count);
    assert_eq!(recent[0].content, format!("Record {}", threshold + 1));
}

#[test]
fn test_cleanup_old_records_when_favorited_record_present_counts_only_ordinary_records() {
    let repo = create_test_repo();

    let favorite = repo
        .save_text("Old favorite".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    for index in 2..=6 {
        repo.save_text(format!("Record {index}"))
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
    }

    repo.toggle_favorite(favorite.id)
        .expect("Failed to favorite record");

    let removed = repo.cleanup_old_records(3).expect("Failed to clean up");
    assert_eq!(removed, 2);
    assert_eq!(repo.count(), 4);
    assert!(
        repo.get_by_id(favorite.id)
            .expect("Failed to get by id")
            .is_some()
    );
}

#[test]
fn test_favorite_toggle_record_updates_membership() {
    let repo = create_test_repo();
    let record = repo
        .save_text("Favorite me".to_string())
        .expect("Failed to save");

    assert!(
        repo.favorite_ids()
            .expect("Failed to load favorite ids")
            .is_empty()
    );

    let is_favorite = repo
        .toggle_favorite(record.id)
        .expect("Failed to add favorite");
    assert!(is_favorite);

    let favorite_ids = repo.favorite_ids().expect("Failed to load favorite ids");
    assert_eq!(favorite_ids, vec![record.id]);

    let is_favorite = repo
        .toggle_favorite(record.id)
        .expect("Failed to remove favorite");
    assert!(!is_favorite);
    assert!(
        repo.favorite_ids()
            .expect("Failed to load favorite ids")
            .is_empty()
    );
}

#[test]
fn test_delete_favorite_record_removes_membership() {
    let repo = create_test_repo();
    let record = repo
        .save_text("Favorite delete".to_string())
        .expect("Failed to save");

    repo.toggle_favorite(record.id)
        .expect("Failed to favorite record");
    assert_eq!(
        repo.favorite_ids().expect("Failed to load favorite ids"),
        vec![record.id]
    );

    let deleted = repo.delete(record.id).expect("Failed to delete");
    assert!(deleted);
    assert!(
        repo.favorite_ids()
            .expect("Failed to load favorite ids")
            .is_empty()
    );
}

#[test]
fn test_delete() {
    let repo = create_test_repo();

    let record = repo
        .save_text("To be deleted".to_string())
        .expect("Failed to save");
    assert_eq!(repo.count(), 1);

    let deleted = repo.delete(record.id).expect("Failed to delete");
    assert!(deleted);
    assert_eq!(repo.count(), 0);

    let deleted_again = repo.delete(record.id).expect("Failed to delete");
    assert!(!deleted_again);
}

#[test]
fn test_delete_when_record_is_rich_text_removes_sidecar_files() {
    let (_temp_dir, repo) = create_test_repo_with(memory_backend_factory);

    let record = repo
        .save_rich_text(
            "hello".to_string(),
            Some("<p>hello</p>"),
            Some("{\\rtf1 hello}"),
        )
        .expect("Failed to save rich text");
    let meta = record
        .rich_text_meta
        .clone()
        .expect("Rich text metadata should be present");
    let html_path = meta.html_path.expect("HTML path should be present");
    let rtf_path = meta.rtf_path.expect("RTF path should be present");

    repo.delete(record.id)
        .expect("Failed to delete rich text record");

    assert!(!std::path::Path::new(&html_path).exists());
    assert!(!std::path::Path::new(&rtf_path).exists());
}

#[test]
fn test_delete_nonexistent() {
    let repo = create_test_repo();

    let result = repo.delete(999_999_999).expect("Failed to delete");
    assert!(!result);
}

#[test]
fn test_clear() {
    let repo = create_test_repo();

    repo.save_text("One".to_string()).expect("Failed to save");
    repo.save_text("Two".to_string()).expect("Failed to save");
    repo.save_text("Three".to_string()).expect("Failed to save");
    assert_eq!(repo.count(), 3);

    repo.clear().expect("Failed to clear");
    assert_eq!(repo.count(), 0);
}

#[test]
fn test_clear_ordinary_records_when_special_records_present_preserves_them() {
    let repo = create_test_repo();

    let pinned = repo
        .save_text("Pinned".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let favorite = repo
        .save_text("Favorite".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let pinned_and_favorite = repo
        .save_text("Pinned Favorite".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let ordinary_one = repo
        .save_text("Ordinary One".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let ordinary_two = repo
        .save_text("Ordinary Two".to_string())
        .expect("Failed to save");

    repo.toggle_pin(pinned.id).expect("Failed to pin record");
    repo.toggle_favorite(favorite.id)
        .expect("Failed to favorite record");
    repo.toggle_pin(pinned_and_favorite.id)
        .expect("Failed to pin record");
    repo.toggle_favorite(pinned_and_favorite.id)
        .expect("Failed to favorite record");

    let removed = repo
        .clear_ordinary_records()
        .expect("Failed to clear ordinary records");

    assert_eq!(removed, 2);
    assert_eq!(repo.count(), 3);
    assert!(
        repo.get_by_id(pinned.id)
            .expect("Failed to load pinned")
            .is_some()
    );
    assert!(
        repo.get_by_id(favorite.id)
            .expect("Failed to load favorite")
            .is_some()
    );
    assert!(
        repo.get_by_id(pinned_and_favorite.id)
            .expect("Failed to load pinned favorite")
            .is_some()
    );
    assert!(
        repo.get_by_id(ordinary_one.id)
            .expect("Failed to load ordinary")
            .is_none()
    );
    assert!(
        repo.get_by_id(ordinary_two.id)
            .expect("Failed to load ordinary")
            .is_none()
    );
    assert_eq!(
        repo.favorite_ids().expect("Failed to load favorite ids"),
        vec![favorite.id, pinned_and_favorite.id]
    );
}

#[test]
fn test_clear_ordinary_records_when_no_ordinary_records_returns_zero() {
    let repo = create_test_repo();

    let pinned = repo
        .save_text("Pinned".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let favorite = repo
        .save_text("Favorite".to_string())
        .expect("Failed to save");

    repo.toggle_pin(pinned.id).expect("Failed to pin record");
    repo.toggle_favorite(favorite.id)
        .expect("Failed to favorite record");

    let removed = repo
        .clear_ordinary_records()
        .expect("Failed to clear ordinary records");

    assert_eq!(removed, 0);
    assert_eq!(repo.count(), 2);
    assert!(
        repo.get_by_id(pinned.id)
            .expect("Failed to load pinned")
            .is_some()
    );
    assert!(
        repo.get_by_id(favorite.id)
            .expect("Failed to load favorite")
            .is_some()
    );
}

#[test]
fn test_clear_empty_repo() {
    let repo = create_test_repo();

    repo.clear().expect("Failed to clear empty repo");
    assert_eq!(repo.count(), 0);
}

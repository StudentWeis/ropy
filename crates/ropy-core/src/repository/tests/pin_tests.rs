#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Tests for pin behavior and pinned-aware cleanup.

use std::{thread, time::Duration};

use crate::repository::test_helpers::{create_test_repo, load_display_records};

#[test]
fn test_toggle_pin() {
    let repo = create_test_repo();
    let record = repo
        .save_text("Pin me".to_string())
        .expect("Failed to save");

    assert!(!record.pinned);

    repo.toggle_pin(record.id).expect("Failed to toggle pin");
    let pinned = repo
        .get_by_id(record.id)
        .expect("Failed to get")
        .expect("not found");
    assert!(pinned.pinned);

    repo.toggle_pin(record.id).expect("Failed to toggle pin");
    let unpinned = repo
        .get_by_id(record.id)
        .expect("Failed to get")
        .expect("not found");
    assert!(!unpinned.pinned);
}

#[test]
fn test_pinned_records_appear_first() {
    let repo = create_test_repo();

    repo.save_text("First".to_string()).expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let second = repo
        .save_text("Second".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    repo.save_text("Third".to_string()).expect("Failed to save");

    repo.toggle_pin(second.id).expect("Failed to toggle pin");

    let recent = load_display_records(&repo, 10);
    assert_eq!(recent[0].content, "Second");
    assert_eq!(recent[1].content, "Third");
    assert_eq!(recent[2].content, "First");
}

#[test]
fn test_multiple_pinned_ordering() {
    let repo = create_test_repo();

    let r1 = repo.save_text("Alpha".to_string()).expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    repo.save_text("Beta".to_string()).expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let r3 = repo.save_text("Gamma".to_string()).expect("Failed to save");

    repo.toggle_pin(r1.id).expect("Failed to toggle pin");
    repo.toggle_pin(r3.id).expect("Failed to toggle pin");

    let recent = load_display_records(&repo, 10);
    assert_eq!(recent[0].content, "Gamma");
    assert_eq!(recent[1].content, "Alpha");
    assert_eq!(recent[2].content, "Beta");
}

#[test]
fn test_cleanup_skips_pinned() {
    let repo = create_test_repo();

    let r1 = repo
        .save_text("Old pinned".to_string())
        .expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    for i in 2..=6 {
        repo.save_text(format!("Record {i}"))
            .expect("Failed to save");
        thread::sleep(Duration::from_millis(10));
    }

    repo.toggle_pin(r1.id).expect("Failed to toggle pin");
    assert_eq!(repo.count(), 6);

    let removed = repo.cleanup_old_records(3).expect("Failed to clean up");
    assert_eq!(removed, 2);
    assert_eq!(repo.count(), 4);
    let pinned = repo
        .get_by_id(r1.id)
        .expect("Failed to get")
        .expect("Pinned record should still exist");
    assert!(pinned.pinned);
}

#[test]
fn test_cleanup_keeps_pinned_when_not_enough_unpinned() {
    let repo = create_test_repo();

    let r1 = repo.save_text("A".to_string()).expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let r2 = repo.save_text("B".to_string()).expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    let r3 = repo.save_text("C".to_string()).expect("Failed to save");
    thread::sleep(Duration::from_millis(10));
    repo.save_text("D".to_string()).expect("Failed to save");

    repo.toggle_pin(r1.id).expect("Failed to toggle pin");
    repo.toggle_pin(r2.id).expect("Failed to toggle pin");
    repo.toggle_pin(r3.id).expect("Failed to toggle pin");

    let removed = repo.cleanup_old_records(2).expect("Failed to clean up");
    assert_eq!(removed, 0);
    assert_eq!(repo.count(), 4);
}

#[test]
fn test_toggle_pin_nonexistent() {
    let repo = create_test_repo();

    let result = repo.toggle_pin(999_999_999);
    assert!(result.is_err());

    let err_msg = format!("{}", result.expect_err("Should be error"));
    assert!(err_msg.contains("record not found"));
}

#[test]
fn test_double_toggle_pin() {
    let repo = create_test_repo();

    let record = repo.save_text("Test".to_string()).expect("Failed to save");
    assert!(!record.pinned);

    repo.toggle_pin(record.id).expect("Failed to toggle");
    let pinned = repo
        .get_by_id(record.id)
        .expect("Failed to get")
        .expect("Should exist");
    assert!(pinned.pinned);

    repo.toggle_pin(record.id).expect("Failed to toggle");
    let unpinned = repo
        .get_by_id(record.id)
        .expect("Failed to get")
        .expect("Should exist");
    assert!(!unpinned.pinned);

    repo.toggle_pin(record.id).expect("Failed to toggle");
    let pinned_again = repo
        .get_by_id(record.id)
        .expect("Failed to get")
        .expect("Should exist");
    assert!(pinned_again.pinned);
}

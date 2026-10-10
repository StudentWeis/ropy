use crate::repository::{
    ClipboardRepository,
    backend::{KvTree, StorageBackend, TIME_INDEX_TREE, memory::MemoryBackend},
};

#[test]
fn test_display_record_read_failure_returns_error_and_preserves_history() {
    use crate::repository::backend::RECORDS_TREE;
    let dir = tempfile::tempdir().expect("fixture directory");
    let backend = MemoryBackend::new();
    let repo = ClipboardRepository::from_backend(backend.clone(), dir.path().join("images"))
        .expect("repository");
    let record = repo.save_text("retained".into()).expect("save");
    repo.toggle_pin(record.id).expect("pin");
    backend
        .open_tree(RECORDS_TREE)
        .expect("tree")
        .fail_next_get();
    assert!(repo.get_display_snapshot(10).is_err());
    let (records, favorites) = repo
        .get_display_snapshot(10)
        .expect("recovery")
        .into_parts();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].content, "retained");
    assert_eq!(records[0].id, record.id);
    assert!(records[0].pinned);
    assert!(favorites.is_empty());
    assert_eq!(repo.count(), 1);
}

#[test]
fn test_display_corrupt_record_skips_only_bad_payload() {
    use crate::repository::backend::RECORDS_TREE;
    let dir = tempfile::tempdir().expect("fixture directory");
    let backend = MemoryBackend::new();
    let repo = ClipboardRepository::from_backend(backend.clone(), dir.path().join("images"))
        .expect("repository");
    let good = repo.save_text("retained".into()).expect("save");
    let bad = repo.save_text("corrupt".into()).expect("save");
    backend
        .open_tree(RECORDS_TREE)
        .expect("tree")
        .insert(&bad.id.to_be_bytes(), b"broken")
        .expect("corrupt payload");
    let records = repo
        .get_display_records(10)
        .expect("display readable records");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].id, good.id);
    assert_eq!(repo.count(), 2);
}

#[test]
fn test_save_failed_commit_preserves_record_and_index() {
    let dir = tempfile::tempdir().expect("fixture directory");
    let backend = MemoryBackend::new();
    let repo = ClipboardRepository::from_backend(backend.clone(), dir.path().join("images"))
        .expect("repository");
    let before = repo.save_text("same".into()).expect("initial save");
    backend.fail_next_batch();
    assert!(repo.save_text("same".into()).is_err());
    let after = repo.get_by_id(before.id).expect("query").expect("record");
    assert_eq!(after.created_at, before.created_at);
    assert_eq!(repo.get_display_records(10).expect("display").len(), 1);
}

#[test]
fn test_reopen_missing_index_repairs_existing_record() {
    let dir = tempfile::tempdir().expect("fixture directory");
    let backend = MemoryBackend::new();
    let repo = ClipboardRepository::from_backend(backend.clone(), dir.path().join("images"))
        .expect("repository");
    let record = repo.save_text("retained".into()).expect("save");
    backend
        .open_tree(TIME_INDEX_TREE)
        .expect("tree")
        .clear()
        .expect("damage index");
    drop(repo);
    let reopened =
        ClipboardRepository::from_backend(backend, dir.path().join("images")).expect("reopen");
    assert_eq!(
        reopened
            .get_display_records(10)
            .expect("display")
            .first()
            .map(|r| r.id),
        Some(record.id)
    );
}

#[rstest::rstest]
#[case(0)]
#[case(1)]
#[case(2)]
#[case(3)]
#[case(4)]
fn test_redb_recopy_aborted_write_reopen_preserves_snapshot(#[case] fail_after: usize) {
    use crate::repository::backend::redb::redb_backend_factory;
    let dir = tempfile::tempdir().expect("fixture directory");
    let path = dir.path().join("fixture.redb");
    let repo = ClipboardRepository::init(&path, dir.path().join("images"), redb_backend_factory)
        .expect("repository");
    let record = repo.save_text("same".into()).expect("save");
    repo.toggle_pin(record.id).expect("pin");
    repo.toggle_favorite(record.id).expect("favorite");
    let before = repo.get_by_id(record.id).expect("query").expect("record");
    repo.backend.fail_write_after(fail_after);
    assert!(repo.save_text("same".into()).is_err());
    // Inspect before reopening so startup repair cannot conceal a failed transaction.
    assert_eq!(
        repo.get_by_id(record.id)
            .expect("query")
            .expect("record")
            .created_at,
        before.created_at
    );
    assert_eq!(repo.get_display_records(0).expect("display").len(), 1);
    drop(repo);
    let reopened =
        ClipboardRepository::init(&path, dir.path().join("images"), redb_backend_factory)
            .expect("reopen");
    let after = reopened
        .get_display_records(0)
        .expect("display")
        .pop()
        .expect("record");
    assert_eq!(after.created_at, before.created_at);
    assert!(after.pinned);
    assert_eq!(reopened.favorite_ids().expect("favorites"), vec![record.id]);
}

#[rstest::rstest]
#[case(0)]
#[case(1)]
#[case(2)]
#[case(3)]
fn test_redb_new_record_aborted_write_leaves_no_record_or_index(#[case] fail_after: usize) {
    use crate::repository::backend::{TIME_INDEX_LOOKUP_TREE, redb::redb_backend_factory};
    let (dir, repo) = crate::repository::test_helpers::create_test_repo_with(redb_backend_factory);
    repo.backend.fail_write_after(fail_after);
    assert!(repo.save_text("new".into()).is_err());
    assert_eq!(repo.count(), 0);
    for tree in [TIME_INDEX_TREE, TIME_INDEX_LOOKUP_TREE] {
        assert_eq!(repo.backend.open_tree(tree).expect("tree").len(), 0);
    }
    drop(repo);
    let reopened = ClipboardRepository::init(
        &dir.path().join("test.db"),
        dir.path().join("images"),
        redb_backend_factory,
    )
    .expect("reopen");
    assert_eq!(reopened.count(), 0);
}

#[test]
fn test_repository_concurrent_recopy_pin_delete_keeps_indexes_consistent() {
    use std::sync::{Arc, Barrier};

    use crate::repository::backend::{TIME_INDEX_LOOKUP_TREE, redb::redb_backend_factory};
    let (_dir, repo) = crate::repository::test_helpers::create_test_repo_with(redb_backend_factory);
    let repo = Arc::new(repo);
    let barrier = Arc::new(Barrier::new(2));
    for _ in 0..30 {
        let record = repo.save_text("same".into()).expect("save");
        let saver = {
            let repo = repo.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                repo.save_text("same".into()).expect("recopy");
            })
        };
        barrier.wait();
        repo.toggle_pin(record.id).expect("pin");
        saver.join().expect("saver");
        assert!(
            repo.get_by_id(record.id)
                .expect("query")
                .expect("record")
                .pinned
        );
        assert_eq!(repo.cleanup_old_records(0).expect("cleanup"), 0);
        let saver = {
            let repo = repo.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                repo.save_text("same".into()).expect("recopy during delete");
            })
        };
        barrier.wait();
        repo.delete(record.id).expect("delete");
        saver.join().expect("saver");
        let count = repo.count();
        assert_eq!(repo.get_display_records(10).expect("display").len(), count);
        for tree in [TIME_INDEX_TREE, TIME_INDEX_LOOKUP_TREE] {
            assert_eq!(repo.backend.open_tree(tree).expect("tree").len(), count);
        }
        repo.delete(record.id).expect("reset fixture");
    }
}

#[test]
fn test_reopen_unreadable_record_preserves_other_history() {
    use crate::repository::backend::RECORDS_TREE;
    let dir = tempfile::tempdir().expect("fixture directory");
    let backend = MemoryBackend::new();
    let repo = ClipboardRepository::from_backend(backend.clone(), dir.path().join("images"))
        .expect("repository");
    let record = repo.save_text("readable".into()).expect("save");
    backend
        .open_tree(RECORDS_TREE)
        .expect("tree")
        .insert(&99_u64.to_be_bytes(), b"broken")
        .expect("corrupt fixture");
    drop(repo);
    let reopened = ClipboardRepository::from_backend(backend.clone(), dir.path().join("images"))
        .expect("reopen preserves readable history");
    assert_eq!(
        reopened.get_display_records(10).expect("display")[0].id,
        record.id
    );
    assert_eq!(
        backend
            .open_tree(RECORDS_TREE)
            .expect("tree")
            .get(&99_u64.to_be_bytes())
            .expect("query"),
        Some(b"broken".to_vec())
    );
}

#[test]
fn test_redb_reopen_inconsistent_indexes_repairs_without_losing_pin_or_favorite() {
    use crate::repository::{
        backend::{TIME_INDEX_LOOKUP_TREE, redb::redb_backend_factory},
        time_index::TimeIndex,
    };
    let (dir, repo) = crate::repository::test_helpers::create_test_repo_with(redb_backend_factory);
    let record = repo.save_text("retained".into()).expect("save");
    repo.toggle_pin(record.id).expect("pin");
    repo.toggle_favorite(record.id).expect("favorite");
    let entries = repo.backend.open_tree(TIME_INDEX_TREE).expect("index");
    entries.clear().expect("damage index");
    entries
        .insert(
            &TimeIndex::<crate::repository::backend::redb::RedbTree>::encode_key(1, 99),
            &[0, 0],
        )
        .expect("orphan index");
    repo.backend
        .open_tree(TIME_INDEX_LOOKUP_TREE)
        .expect("lookup")
        .insert(&record.id.to_be_bytes(), b"invalid timestamp")
        .expect("damage lookup");
    drop(entries);
    drop(repo);
    let reopened = ClipboardRepository::init(
        &dir.path().join("test.db"),
        dir.path().join("images"),
        redb_backend_factory,
    )
    .expect("reopen");
    let display = reopened.get_display_records(0).expect("display");
    assert_eq!(display.len(), 1);
    assert_eq!(display[0].id, record.id);
    assert!(display[0].pinned);
    assert_eq!(reopened.favorite_ids().expect("favorites"), vec![record.id]);
    for tree in [TIME_INDEX_TREE, TIME_INDEX_LOOKUP_TREE] {
        assert_eq!(reopened.backend.open_tree(tree).expect("tree").len(), 1);
    }
}

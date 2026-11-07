#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use rstest::rstest;

use super::{MemoryBackend, recover_lock};
use crate::repository::backend::{KvTree, StorageBackend, TreeKey, TreeWrite};

#[derive(Clone, Copy)]
enum Batch {
    Write,
    Remove,
    Clear,
}

#[derive(Clone, Copy)]
enum Read {
    Get,
    Len,
    Ascending,
    Descending,
}

#[rstest]
#[case::write(Batch::Write)]
#[case::remove(Batch::Remove)]
#[case::clear(Batch::Clear)]
fn test_batch_in_progress_reads_wait_for_commit(
    #[case] batch: Batch,
    #[values(Read::Get, Read::Len, Read::Ascending, Read::Descending)] read: Read,
) {
    let backend = MemoryBackend::new();
    let first = backend.open_tree("first").unwrap();
    let second = backend.open_tree("second").unwrap();
    first.insert(b"key", b"original").unwrap();
    second.insert(b"key", b"original").unwrap();

    // Pause the actual batch at its second tree, after it has changed the first.
    let second_entries = recover_lock(second.tree.entries.write());
    let writer = thread::spawn(move || match batch {
        Batch::Write => backend.write_batch(&[
            TreeWrite::Insert(TreeKey::new("first", b"temporary"), b"uncommitted"),
            TreeWrite::Insert(TreeKey::new("second", b"key"), b"committed"),
            TreeWrite::Remove(TreeKey::new("first", b"temporary")),
            TreeWrite::Insert(TreeKey::new("first", b"key"), b"committed"),
        ]),
        Batch::Remove => backend
            .remove_batch(&[
                TreeKey::new("first", b"key"),
                TreeKey::new("second", b"key"),
            ])
            .map(|_| ()),
        Batch::Clear => backend.clear_batch(&["first", "second"]),
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    while recover_lock(first.tree.entries.read()).len() == 1 {
        assert!(Instant::now() < deadline, "batch never reached second tree");
        thread::yield_now();
    }

    let (started_tx, started_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let reader = thread::spawn(move || {
        started_tx.send(()).unwrap();
        let present = matches!(batch, Batch::Write);
        let committed = match read {
            Read::Get => first.get(b"key").unwrap() == present.then(|| b"committed".to_vec()),
            Read::Len => first.len().unwrap() == usize::from(present),
            Read::Ascending | Read::Descending => {
                let mut entries = Vec::new();
                let mut callback = |key: &[u8], value: &[u8]| {
                    entries.push((key.to_vec(), value.to_vec()));
                    true
                };
                match read {
                    Read::Ascending => first.scan_ascending(&mut callback).unwrap(),
                    _ => first.scan_descending(&mut callback).unwrap(),
                }
                let expected = if present {
                    vec![(b"key".to_vec(), b"committed".to_vec())]
                } else {
                    Vec::new()
                };
                entries == expected
            }
        };
        result_tx.send(committed).unwrap();
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let before_commit = result_rx.recv_timeout(Duration::from_millis(100));
    drop(second_entries);
    writer.join().unwrap().unwrap();
    reader.join().unwrap();

    assert!(
        matches!(before_commit, Err(mpsc::RecvTimeoutError::Timeout)),
        "read exposed an in-progress batch"
    );
    assert!(result_rx.recv_timeout(Duration::from_secs(2)).unwrap());
}

#[rstest]
#[case::ascending(false)]
#[case::descending(true)]
fn test_scan_callback_reads_other_tree_and_mutates_source_snapshot(#[case] descending: bool) {
    let backend = MemoryBackend::new();
    let source = backend.open_tree("source").unwrap();
    let other = backend.open_tree("other").unwrap();
    source.insert(b"a", b"one").unwrap();
    source.insert(b"b", b"two").unwrap();
    other.insert(b"key", b"other value").unwrap();
    let (result_tx, result_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let mut visited = Vec::new();
        let mut callback = |key: &[u8], value: &[u8]| {
            assert_eq!(
                other.get(b"key").unwrap().as_deref(),
                Some(&b"other value"[..])
            );
            source.remove(b"a").unwrap();
            source.insert(b"c", b"new").unwrap();
            visited.push((key.to_vec(), value.to_vec()));
            true
        };
        if descending {
            source.scan_descending(&mut callback).unwrap();
        } else {
            source.scan_ascending(&mut callback).unwrap();
        }
        result_tx.send(visited).unwrap();
    });

    let visited = result_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("scan callback deadlocked while accessing trees");
    worker.join().unwrap();
    let mut expected = vec![
        (b"a".to_vec(), b"one".to_vec()),
        (b"b".to_vec(), b"two".to_vec()),
    ];
    if descending {
        expected.reverse();
    }
    assert_eq!(visited, expected);
}

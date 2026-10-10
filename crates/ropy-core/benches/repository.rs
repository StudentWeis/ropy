//! Real redb storage latency with a fixed on-disk starting state.
#![expect(
    clippy::expect_used,
    reason = "A failed benchmark must abort rather than report timings"
)]

use std::{hint::black_box, time::Duration};

use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use ropy_core::repository::ClipboardRepository;

#[path = "../../../scripts/bench/fixture.rs"]
pub mod fixture;

fn repository(criterion: &mut Criterion) {
    let directory = tempfile::tempdir().expect("fixture directory");
    let database = directory.path().join("seed.redb");
    let repo = ClipboardRepository::open(&database, directory.path().join("images"))
        .expect("open seed repository");
    fixture::seed(&repo).expect("seed repository");
    drop(repo);

    // Copy the closed snapshot and open it outside each timed write. Neither
    // insertion nor recopy can grow state across benchmark iterations.
    for (name, index) in [("insert", 200), ("dedup", 0)] {
        criterion.bench_function(name, |bencher| {
            bencher.iter_batched_ref(
                || {
                    let temporary = tempfile::tempdir().expect("iteration directory");
                    let path = temporary.path().join("clipboard.redb");
                    std::fs::copy(&database, &path).expect("copy fixture");
                    let repo = ClipboardRepository::open(&path, temporary.path().join("images"))
                        .expect("open iteration repository");
                    (repo, temporary, fixture::text(index))
                },
                |(repo, _, text)| {
                    black_box(repo.save_text(std::mem::take(text)).expect("save text"));
                },
                BatchSize::PerIteration,
            );
        });
    }
    let repo = ClipboardRepository::open(&database, directory.path().join("images"))
        .expect("open read repository");
    criterion.bench_function("read_100", |bencher| {
        bencher.iter(|| black_box(repo.get_display_records(100).expect("read records")));
    });
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .sample_size(10)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(2));
    targets = repository
}
criterion_main!(benches);

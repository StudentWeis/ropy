//! Deterministic filtering workloads; fixture generation is outside the timer.
#![expect(
    clippy::expect_used,
    reason = "Invalid fixtures must abort the benchmark"
)]

use std::{collections::HashSet, hint::black_box, time::Duration};

use criterion::{
    BenchmarkGroup, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
    measurement::WallTime,
};
use ropy_core::{
    repository::{ClipboardRecord, ContentType},
    search::{ContentFilter, SearchOptions, filter_records_by_query},
};

const ASCII: &str = "Project clipboard history: Rust code, https://example.org/docs, 0123456789. ";
const UNICODE: &str = "剪贴板历史记录与开发文档，大小写 École Straße ΟΣ İSTANBUL 🙂。 ";

fn search(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("search_v1");
    for count in [100, 1_000, 10_000] {
        for name in [
            "ascii_hit",
            "ascii_miss",
            "ascii_sensitive",
            "prefix",
            "whole_word",
            "whole_word_sensitive",
            "whole_word_dense",
            "unicode_hit",
            "unicode_miss",
            "unicode_whole_word",
            "files",
            "legacy_paths",
            "empty",
            "images",
            "favorites",
        ] {
            let (records, query, options, filter, favorites_only) = fixture(name, count);
            let favorites = (0..count)
                .filter(|ix| ix % 20 == 0)
                .map(|ix| ix as u64)
                .collect::<HashSet<_>>();
            let expected: Vec<usize> = (0..count)
                .filter(|ix| match name {
                    "ascii_miss" | "unicode_miss" => false,
                    "empty" | "images" => true,
                    "favorites" => ix % 20 == 0,
                    _ => ix % 10 == 0,
                })
                .collect();
            assert_eq!(
                filter_records_by_query(
                    &records,
                    query,
                    filter,
                    options,
                    &favorites,
                    favorites_only
                ),
                expected,
                "{name}/{count}"
            );
            group.throughput(Throughput::Elements(count as u64));
            group.bench_with_input(
                BenchmarkId::new(name, count),
                &records,
                |bencher, records| {
                    bencher.iter(|| {
                        black_box(filter_records_by_query(
                            black_box(records),
                            black_box(query),
                            filter,
                            options,
                            black_box(&favorites),
                            favorites_only,
                        ));
                    });
                },
            );
        }
    }
    benchmark_long_records(&mut group);
    group.finish();
}

fn benchmark_long_records(group: &mut BenchmarkGroup<'_, WallTime>) {
    // Long records and a long shared prefix expose repeated prefix comparisons.
    for name in ["long_text", "long_prefix"] {
        let mut options = SearchOptions::default();
        options.whole_word = true;
        let query = if name == "long_prefix" {
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaab"
        } else {
            "needle42"
        };
        let body = if name == "long_prefix" {
            "a".repeat(65_536)
        } else {
            ASCII.repeat(1_024)
        };
        let records = text_records(100, &body, "Needle42");
        let expected = if name == "long_prefix" {
            vec![]
        } else {
            (0..100).step_by(10).collect()
        };
        assert_eq!(
            filter_records_by_query(
                &records,
                query,
                ContentFilter::All,
                options,
                &HashSet::new(),
                false
            ),
            expected
        );
        group.throughput(Throughput::Elements(100));
        group.bench_with_input(BenchmarkId::new(name, 100), &records, |bencher, records| {
            let favorites = HashSet::new();
            bencher.iter(|| {
                black_box(filter_records_by_query(
                    black_box(records),
                    black_box(query),
                    ContentFilter::All,
                    options,
                    &favorites,
                    false,
                ));
            });
        });
    }
}

fn fixture(
    name: &str,
    count: usize,
) -> (
    Vec<ClipboardRecord>,
    &'static str,
    SearchOptions,
    ContentFilter,
    bool,
) {
    let mut options = SearchOptions::default();
    options.case_sensitive = matches!(name, "ascii_sensitive" | "whole_word_sensitive");
    options.whole_word = matches!(
        name,
        "whole_word" | "whole_word_sensitive" | "whole_word_dense" | "unicode_whole_word"
    );
    let query = match name {
        "ascii_miss" | "unicode_miss" => "absent-needle99",
        "ascii_sensitive" | "whole_word_sensitive" => "Needle42",
        "prefix" => "need",
        "unicode_hit" | "unicode_whole_word" => "搜索",
        "files" | "legacy_paths" => "report final",
        "empty" => "",
        _ => "needle42",
    };
    let body = match name {
        "unicode_hit" | "unicode_miss" | "unicode_whole_word" => UNICODE.repeat(8),
        "whole_word_dense" => "needle42_x ".repeat(90),
        _ => ASCII.repeat(14),
    };
    let marker = if name.starts_with("unicode") {
        "搜索"
    } else {
        "Needle42"
    };
    let mut records = text_records(count, &body, marker);
    let filter = match name {
        "files" | "legacy_paths" => ContentFilter::Files,
        "images" => ContentFilter::Image,
        _ => ContentFilter::All,
    };
    for (ix, record) in records.iter_mut().enumerate() {
        match name {
            "files" => {
                record.content_type = ContentType::FilePath;
                let first = format!("/tmp/project-{ix}/notes.txt");
                let second = format!("/tmp/project-{ix}/报告.pdf");
                let third = if ix % 10 == 0 {
                    "file:///tmp/Report%20Final.pdf"
                } else {
                    "/tmp/archive.zip"
                };
                record.content = serde_json::to_string(&[first.as_str(), second.as_str(), third])
                    .expect("serialize fixture");
            }
            "legacy_paths" => {
                record.content_type = ContentType::FilePath;
                record.content = if ix % 10 == 0 {
                    format!("file://localhost/tmp/{ix}/Report%20Final.pdf")
                } else {
                    format!("/tmp/{ix}/notes.txt")
                };
            }
            "images" => record.content_type = ContentType::Image,
            _ => {}
        }
    }
    (records, query, options, filter, name == "favorites")
}

fn text_records(count: usize, body: &str, marker: &str) -> Vec<ClipboardRecord> {
    let timestamp = chrono::DateTime::from_timestamp(1_700_000_000, 0)
        .expect("fixture timestamp")
        .with_timezone(&chrono::Local);
    (0..count)
        .map(|ix| {
            let content = if ix % 10 == 0 {
                format!("{ix}: {body} {marker}.")
            } else {
                format!("{ix}: {body}.")
            };
            let content_type = if ix % 2 == 0 {
                ContentType::Text
            } else {
                ContentType::RichText
            };
            ClipboardRecord::new(ix as u64, content, timestamp, content_type)
        })
        .collect()
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(30).warm_up_time(Duration::from_millis(500)).measurement_time(Duration::from_secs(1));
    targets = search
}
criterion_main!(benches);

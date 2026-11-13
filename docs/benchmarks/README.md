# Benchmark evidence

These records describe specific measured commits and environments, not the current checkout or every published release. Use [Release benchmark baseline](../benchmarks.md) for collection and comparison rules, and [Search benchmark and implementation](../search-performance.md) for filtering semantics and timing boundaries.

| Record | Scope and limits |
| --- | --- |
| [0.5.7 macOS arm64 baseline](0.5.7-Darwin-arm64-0845036d448e-408f9764.md) and its [raw result](0.5.7-Darwin-arm64-0845036d448e-408f9764.json) | Storage, executable size and isolated hidden-desktop RSS at commit `0845036`; no downloaded package measured |
| [2026-10-20 search comparison](search/2026-10-20-macos-arm64.md) and its [raw result](search/2026-10-20-macos-arm64.json) | 47 filtering workloads, prototypes and paired rechecks on one Mac; not database, rendering, RSS or cross-platform latency |
| [2026-11-08 release-size decision](release-size-2026-11-08/README.md) | Linux experiments informing the explicit XZ9 policy for the custom macOS update archive; not a measured macOS package saving |
| [Frozen build-size CSV](../data/build_sizes.csv) | Historical archive with insufficient environment metadata for v1 comparisons; no longer updated |

Keep generated Markdown/JSON pairs together and do not relabel old measurements. `make bench-record` owns generated storage/RSS reports; preserve raw samples and measured revisions. New runs need distinct output directories and the collection conditions specified by the corresponding guide.

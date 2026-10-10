# Release benchmark baseline

The v1 suite is informational: it records size, three core storage operations,
and isolated hidden-desktop RSS. It does not block PRs or releases.

## Run on the reference Mac

Use Python 3.11+, the pinned Rust toolchain, and an unlocked macOS desktop.
Keep the machine, power mode, OS and background workload consistent. Run one
benchmark at a time, after compilation and other heavy work have stopped.

```sh
make bench
make bench-compare BASELINE=/path/to/previous/result.json
```

Outputs go to `target/bench/<version>/`: `result.json`, `report.md`, and
`criterion/` with raw timing samples and statistical estimates. Existing output
directories are refused so a second run cannot overwrite the baseline. For a
repeat or a candidate with the same version:

```sh
make bench BENCH_ARGS='--output /tmp/ropy-bench-repeat'
make bench-compare BASELINE=/path/to/previous/result.json CURRENT=/tmp/ropy-bench-repeat/result.json
```

`make bench` uses Cargo directly, builds the release application, and runs the
core bench profile (which inherits release optimization settings). The first
build may take several minutes. The RSS phase waits for readiness, settles for
10 seconds, then samples the launched PID every second for 30 samples. It never
attaches to a user's running instance. Process failure or invalid readiness
fails the run; it is not reported as a low memory measurement.

Use `BENCH_ARGS='--skip-memory'` for core/size collection without a desktop or
on Linux/Windows. The omitted memory metric is not a zero. Full desktop RSS
collection currently supports macOS only. No CPU or startup latency is measured.

## Fixed workload and interpretation

The shared fixture is `scripts/bench/fixture.rs`: 200 unique ASCII strings,
exactly 1 KiB each, generated from an index without randomness. Fixture version
1 has no images, favorites or pins.

- **insert:** add a new 1 KiB text record to a real redb database containing 200 records.
- **dedup:** save an existing 1 KiB text record again, including timestamp/index updates.
- **read_100:** read the latest 100 display records from the 200-record database.

Each write iteration copies a closed seeded database, opens it, and prepares
the input outside the timer; teardown is also outside the timer. This preserves
the starting record count and avoids measuring growing databases. Writes use
production durability. The returned record is dropped inside the timed write.
Reads reuse an open, warmed repository and include result allocation/drop.
Neither benchmark is a cold filesystem test. Criterion collects 10 statistical
samples after a 1-second warmup with a 2-second measurement target; fixture
preparation means actual elapsed time can be longer. Reports use its median
estimate and 95% confidence interval, not request P95.

**RSS** uses the actual release executable and production hidden board with
100 loaded records and the 200-record redb repository. The `--bench-memory`
entry point runs before normal logging, updater startup, or single-instance
handling. It uses an external temporary directory, default in-memory settings,
and no clipboard listener/writer, tray, global hotkey, autostart registration
or update checks. The window stays hidden and has no activation entry point.
This measures the isolated GUI/storage footprint, **not total normal resident
application memory**. The Python parent terminates only its own child and
removes its temporary database even when sampling fails. RSS is macOS `ps`
RSS in KiB converted to bytes; it is not physical footprint or all GPU memory.

The report compares compatible schema/suite/fixture versions and identical
recorded environments. Differences or missing measurements display
`not comparable` / `not measured`. Treat small changes within measurement noise
as inconclusive. The first run establishes a baseline, not an improvement.
When changing workload or timing semantics, bump `suite_version` and, for data
changes, `fixture_version` in `scripts/bench.py`.

## Package size and release retention

The normal run records release executable bytes and SHA-256. Optionally include
a matching downloaded release package:

```sh
make bench BENCH_ARGS='--package /path/to/ropy-aarch64-apple-darwin.tar.xz'
```

The package must be from the same version, platform, architecture and build you
are evaluating; it is measured as supplied, not rebuilt by the runner. Keep
archive format consistent across comparisons. `result.json` records its name
and hash for auditability.

The existing global release-artifact workflow automatically generates
`bench-sizes.json` alongside `latest.json`; both are attached to the GitHub
Release by the existing release workflow. It records target, archive bytes,
SHA-256 and the main binary's uncompressed size for tar/ZIP archives. It never
extracts archives. DMG files have download size only. Compare each target and
archive kind separately; do not add packages together or mix architectures.

Run the full suite manually once per candidate release on the reference Mac.
Attach its `result.json`, `report.md` and archived `criterion/` directory to that
release. No timing runner, dashboard or automatic upload from the local machine
is introduced. The old `docs/data/build_sizes.csv` and `record_build_size.sh`
remain historical measurements; their missing environment metadata prevents
using them as comparable v1 baselines.

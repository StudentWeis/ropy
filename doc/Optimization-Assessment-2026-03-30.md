**Optimization Assessment (2026-03-30)**

This document records a read-only optimization review of the current Ropy codebase.

## Scope

- Reviewed project structure, runtime flow, repository design, GUI rendering path, clipboard pipeline, and build configuration.
- Verified the current baseline with:
  - `rtk cargo test -- --test-threads=1`
  - `rtk cargo clippy --all-targets --all-features -- -D warnings`
- This review is based on source inspection and baseline validation, not on profiler captures or long-running production traces.

## Current Baseline

- Test suite status: passing (`183` tests)
- Clippy status: passing with warnings denied
- General code health: good
- Main opportunity type: targeted structural and runtime optimizations rather than broad cleanup

## Priority Findings

### 1. Time index updates do not scale well

**Priority:** High

The current time index implementation removes stale entries by scanning the index for an existing record ID before inserting the replacement entry.

- Relevant code:
  - `src/repository/time_index.rs`
  - `TimeIndex::upsert`
  - `TimeIndex::remove_by_id`
- Why it matters:
  - The storage configuration allows much larger repositories than the default runtime cache.
  - At higher record counts, a scan on every deduplicated save becomes an avoidable cost.
- Impact:
  - Acceptable at current defaults.
  - Becomes increasingly expensive if `max_storage_records` is raised significantly.
- Recommendation:
  - Add an auxiliary `id -> timestamp` index, or redesign the index so the old time entry can be removed in near O(1) time.

### 2. Cleanup runs after every successful save

**Priority:** High

After a clipboard record is saved, the foreground update path immediately triggers repository cleanup.

- Relevant code:
  - `src/app.rs`
  - `src/repository/repo.rs`
  - `src/repository/time_index.rs`
- Why it matters:
  - Cleanup is maintenance work, not user-visible work.
  - Running it after every event increases steady-state I/O and index traversal frequency.
- Impact:
  - Small at default limits.
  - More noticeable during high-frequency clipboard activity or when storage limits are increased.
- Recommendation:
  - Debounce cleanup, batch it, or trigger it only when the repository exceeds the storage limit by a buffer.

### 3. `ContentType::FilePath` is not fully implemented in confirm flow

**Priority:** High

The data model and some UI paths already acknowledge file-path records, but the confirm action still contains a `todo!()` branch.

- Relevant code:
  - `src/gui/board/mod.rs`
  - `src/repository/repo.rs`
  - `src/gui/board/records_list.rs`
- Why it matters:
  - This is a correctness risk rather than a pure optimization issue.
  - If a file-path record reaches the confirm flow, it can panic.
- Recommendation:
  - Implement the clipboard write behavior for file paths, or fail gracefully with a user-visible warning instead of panicking.

### 4. Release profile favors binary size over runtime performance

**Priority:** Medium

The release profile currently uses `opt-level = "z"` with `lto = "fat"`.

- Relevant code:
  - `Cargo.toml`
- Why it matters:
  - This is a valid tradeoff for small binaries.
  - It is not the best tradeoff if launch time and runtime responsiveness are a higher priority.
- Recommendation:
  - Decide explicitly whether release builds are optimized for package size or runtime speed.
  - If runtime speed matters more, test `opt-level = 3` or `opt-level = "s"` and compare binary size and responsiveness.

### 5. Search/filter recomputation is simple but not yet a true hotspot

**Priority:** Low to Medium

The board recomputes filtered record indices during render.

- Relevant code:
  - `src/gui/board/mod.rs`
  - `src/gui/board/search.rs`
- Why it matters:
  - Recomputing filters on every render can become expensive when the visible working set grows.
  - Today the default UI history limit is only `100`, so the current implementation is still reasonable.
- Recommendation:
  - Keep the current approach for now unless the UI cache grows materially.
  - If larger history windows are planned, add memoization keyed by query, filter, search options, and favorite membership revision.

## Positive Observations

### 1. Architecture boundaries are clear

The top-level orchestration in `src/app.rs` is separated cleanly from GUI rendering and repository details. This makes future refactors lower risk.

### 2. Repository design is already moving in the right direction

Using a dedicated time index and postcard serialization shows good attention to real storage and retrieval costs.

### 3. Quality gates are already healthy

The current codebase passes both the test suite and strict Clippy checks, which lowers the risk of targeted refactors.

### 4. Concurrency responsibilities are documented

The existing concurrency documentation in `doc/Concurrency-Analysis.md` makes it easier to reason about where to optimize without breaking thread boundaries.

## Suggested Implementation Order

1. Remove the `todo!()` in the file-path confirm flow.
2. Rework the time index update path to avoid scan-based stale-entry removal.
3. Change cleanup from per-save execution to buffered or debounced execution.
4. Revisit release profile tradeoffs after measuring package size versus runtime speed.
5. Only optimize board filtering if the UI record window grows beyond current defaults.

## Notes

- No source changes were made as part of the original review.
- This document records the conclusions of that review for future implementation work.

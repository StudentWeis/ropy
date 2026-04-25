# Code Review — Improvement Opportunities

> Generated: 2026-04-25
> Reviewer: AI code audit (full static analysis of `src/`)
> Basis: project structure, line counts, `#[allow(clippy::...)]` annotations, profiling of hot paths, and architectural patterns.

---

## Summary

The codebase is **well-structured overall** — strong test coverage (363 test cases across 36 files), clean backend abstraction, and a clear contribution SOP. The areas below are about reducing accidental complexity that has accumulated as the feature set grew.

---

## 1. Structural / Architecture

### 1.1 `src/repository/repo.rs` — 1762-line God File ⚠️ High Priority

**Facts:**
- Production code ≈ 460 lines; the remaining 1 300+ lines are `#[cfg(test)]` test cases embedded in the same file.
- The file simultaneously handles: repository construction, schema migration, `save`/`get`/`delete`/`clear`, pin logic, dedup, and rich-text sidecar file management.

**Problems:**
- Longer incremental compile cycles because the entire file recompiles when a single test changes.
- Adding a new `ContentType` requires reading the whole file to feel safe.

**Recommended fix:**
- Move `#[cfg(test)] mod tests` into a `src/repository/tests/` subdirectory, split by theme:
  - `tests/save_tests.rs`, `tests/pin_tests.rs`, `tests/dedup_tests.rs`, `tests/display_tests.rs`
- Extract sidecar file-system logic (`save_image_from_path`, `save_rich_text` FS side-effects) into a new `sidecar.rs`, leaving `repo.rs` responsible only for DB encode/decode.

---

### 1.2 `src/gui/board/records_list.rs` — 1253 lines, 3 concerns mixed ⚠️ High Priority

**Facts:**
- Contains 7 pure `truncate_*` / `estimated_*` text-metric functions.
- Contains 8+ per-record renderers (`render_image_record`, `render_text_record`, `render_file_record`, etc.).
- Contains grid geometry helpers (`build_masonry_layout`, `grid_card_width`, `masonry_placement_is_visible`).

**Problems:**
- Layout, measurement, and rendering are tangled — adding a new `ContentType` UI requires reading the whole file.
- The pure metric functions are untested (buried in a render-heavy file, easy to overlook).

**Recommended fix — split into sub-modules:**
```
records_list/
  mod.rs        — assembly logic only
  metrics.rs    — truncate_*, estimated_*, visible_list_len (pure functions, easy to unit-test)
  masonry.rs    — grid layout geometry
  row.rs        — single-row render
```

---

### 1.3 `RopyBoard` God Struct — 27 fields, 3× `allow(struct_excessive_bools)` ⚠️ Medium

**Facts:**
- `RopyBoard` carries 27 fields. Identical `#[allow(clippy::struct_excessive_bools)]` appears in `mod.rs`, `records_list.rs`, and `settings_editor.rs`.
- UI transient flags: `show_preview`, `show_clear_confirm`, `pinned`, `favorites_only`, `deleting_record`, `grid_auto_reveal_suppressed` — all live directly on the root struct.

**Recommended fix:**
- Group related booleans into sub-structs to make illegal state combinations unrepresentable and remove the `allow` suppressions:
  - `FilterState { content_filter, favorites_only, search_options }`
  - `UiFlags { show_preview, show_clear_confirm, deleting_record, grid_auto_reveal_suppressed }`

---

### 1.4 Storage Backend Consolidation ✅ Completed

**Current state:**
- `Cargo.toml` keeps only `redb = "4.1.0"`.
- `ClipboardRepository::new` always opens `clipboard.redb`.
- The legacy storage backend module and runtime backend switch are gone.
- `ClipboardRepository` and `TimeIndex` now use concrete backend/tree types, with the in-memory backend retained only for tests.

**Follow-up:**
- If profiling still shows storage overhead, the next step is microbenchmarking redb transactions themselves rather than removing trait objects.

---

## 2. Correctness / Robustness

### 2.1 Unbounded Channels in Clipboard Event Pipeline ⚠️ Medium

**Facts (`src/app.rs:41–108`):**
- Both the clipboard event channel and the UI notification channel are `async_channel::unbounded`.
- Every notification triggers a full `get_display_records(max_history_records)` read.

**Problems:**
- Some apps write to the clipboard several times per second; unbounded channels can grow memory without limit.
- Rapid copies cause redundant full-list refreshes.

**Recommended fix:**
- Use **bounded channels** (e.g. capacity 256) and log dropped events.
- Implement **notification coalescing**: drain all pending `()` notifications from the channel before doing a single UI refresh (classic debounce/coalesce pattern).

---

### 2.2 Silent Error Swallowing During Schema Migration 🐛 Low

**Location:** `src/repository/repo.rs::init`

```rust
if images_dir.exists() {
    fs::remove_dir_all(&images_dir).ok();  // error silently discarded
}
```

**Problem:** If deletion fails, orphaned sidecar files remain with no log entry — violates the `thiserror`-first spirit in `AGENTS.md`.

**Recommended fix:**
```rust
if images_dir.exists() {
    if let Err(e) = fs::remove_dir_all(&images_dir) {
        tracing::warn!(error = %e, "failed to remove stale images dir during schema migration");
    }
}
```

## 3. Performance

### 3.2 Storage Dispatch Cleanup ✅ Completed

The repository storage path now uses concrete backend and tree types, so the old database-layer vtable cost is gone from the hot save/get/delete path.

---

## 4. Testability / CI Quality

### 4.1 No Coverage Reporting ♻️ Medium

**Facts:** Neither `scripts/precheck.sh` nor `.github/workflows/ci.yml` runs a coverage tool.

**Recommended fix:**
Add a CI step (can be non-blocking initially):
```bash
cargo llvm-cov --lcov --output-path lcov.info
# upload to Codecov or print diff in PR comment
```

---

### 4.2 All 363 Tests Run Single-Threaded ♻️ Low

**Facts (`scripts/precheck.sh`):**
```bash
cargo test -- --test-threads=1
```

**Problem:** Only tests that share global state (filesystem paths, env vars, global hotkey) actually need serial execution.

**Recommended fix:**
- Add `serial_test` crate as a dev-dependency.
- Annotate only the genuinely stateful tests with `#[serial]`.
- Remove `--test-threads=1` from `precheck.sh` to unlock parallel execution for the majority.

---

### 4.3 `doc/TESTING.md` is Only 5 Lines ♻️ Low

**Facts:** `AGENTS.md` points to `doc/TESTING.md` as the authoritative testing guide, but it contains only a bullet list with no examples.

**Recommended fix:** Expand with:
- A minimal `rstest` parametrised test template.
- A `test_<object>_<scenario>_<expected>` naming example.
- Guidance on when `tempfile` vs `MemoryBackend` is appropriate.

---

## 5. Small, High ROI Cleanups

| # | Location | Issue | Fix |
|---|----------|-------|-----|
| 5.1 | `src/config/settings.rs` (730 lines) | Struct definitions, validation, and default impls mixed together | Split into `settings/mod.rs` + `settings/validate.rs` |
| 5.2 | `src/gui/board/settings_handler.rs` (950 lines) | 20+ `save_xxx` methods in one file | Group into `settings_handler/{hotkey,layout,storage,theme,update}.rs` |
| 5.3 | `records_list.rs:79–118` | 5 thin `truncate_*` wrappers that each call the same core function | Consolidate into a single `truncate(content, limit, options)` |
| 5.4 | CI / `precheck.sh` | `cargo-machete` not run — unused dependencies undetected | Add `cargo machete` to precheck to catch dead dependencies earlier |

---

## Recommended Execution Order

### Tier 1 — Low risk, high return (no API changes)
1. **Refactor**: Split `repo.rs` tests into `src/repository/tests/` subdirectory
2. **Refactor**: Extract `metrics.rs` and `masonry.rs` from `records_list.rs`
3. **Style**: Replace per-function `#[allow(clippy::expect_used)]` with module-level `cfg_attr`
4. **CI**: Remove `--test-threads=1`; add `serial_test` where needed

### Tier 2 — One release cycle
5. **Perf**: Change `[profile.release]` to `opt-level = 3`
6. **Refactor**: Consider a generic `ClipboardRepository` to trim remaining storage vtable overhead
7. **Refactor**: Extract `BoardUiState` sub-structs from `RopyBoard`
8. **CI**: Add `cargo llvm-cov` coverage reporting

### Tier 3 — Requires design discussion
9. **Perf**: Bounded channels + notification coalescing in clipboard event pipeline
10. **Docs**: Expand `doc/TESTING.md` with templates and examples

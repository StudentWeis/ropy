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


### 1.3 `RopyBoard` God Struct — 27 fields, 3× `allow(struct_excessive_bools)` ⚠️ Medium

**Facts:**
- `RopyBoard` carries 27 fields. Identical `#[allow(clippy::struct_excessive_bools)]` appears in `mod.rs`, `records_list.rs`, and `settings_editor.rs`.
- UI transient flags: `show_preview`, `show_clear_confirm`, `pinned`, `favorites_only`, `deleting_record`, `grid_auto_reveal_suppressed` — all live directly on the root struct.

**Recommended fix:**
- Group related booleans into sub-structs to make illegal state combinations unrepresentable and remove the `allow` suppressions:
  - `FilterState { content_filter, favorites_only, search_options }`
  - `UiFlags { show_preview, show_clear_confirm, deleting_record, grid_auto_reveal_suppressed }`

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

## 4. Testability / CI Quality

### 4.1 No Coverage Reporting ♻️ Medium

**Facts:** Neither `scripts/precheck.sh` nor `.github/workflows/ci.yml` runs a coverage tool.

**Recommended fix:**
Add a CI step (can be non-blocking initially):
```bash
cargo llvm-cov --lcov --output-path lcov.info
# upload to Codecov or print diff in PR comment
```

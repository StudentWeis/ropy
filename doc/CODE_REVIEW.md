

### 1.3 `RopyBoard` God Struct — 27 fields, 3× `allow(struct_excessive_bools)` ⚠️ Medium

**Facts:**
- `RopyBoard` carries 27 fields. Identical `#[allow(clippy::struct_excessive_bools)]` appears in `mod.rs`, `records_list.rs`, and `settings_editor.rs`.
- UI transient flags: `show_preview`, `show_clear_confirm`, `pinned`, `favorites_only`, `deleting_record`, `grid_auto_reveal_suppressed` — all live directly on the root struct.

**Recommended fix:**
- Group related booleans into sub-structs to make illegal state combinations unrepresentable and remove the `allow` suppressions:
  - `FilterState { content_filter, favorites_only, search_options }`
  - `UiFlags { show_preview, show_clear_confirm, deleting_record, grid_auto_reveal_suppressed }`

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

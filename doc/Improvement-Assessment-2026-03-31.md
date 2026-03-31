**Improvement Assessment (2026-03-31)**

This document records a comprehensive improvement review of the current Ropy codebase, covering code quality, test coverage, engineering practices, and architectural concerns.

## Priority Findings

### 1. Test coverage is critically low

**Priority:** Critical

The project has almost no unit or integration tests despite `rstest` being configured as a dev-dependency and `AGENTS.md` requiring TDD.

- Modules with no test coverage:
  - `src/app.rs`
  - `src/clipboard/listener.rs`
  - `src/clipboard/writer.rs`
  - `src/clipboard/mod.rs`
  - `src/gui/app.rs`
  - `src/gui/paste.rs`
  - `src/gui/utils.rs`
  - `src/gui/x11.rs`
  - `src/utils/single_instance.rs`
  - `src/main.rs`
- Modules with partial coverage:
  - `src/repository/repo.rs`
  - `src/config/settings.rs`
  - `src/i18n/`
  - `src/updater/`
  - `src/gui/hotkey.rs`
  - `src/gui/tray.rs`
- Why it matters:
  - Core clipboard and repository logic can regress silently.
  - The CI workflow (`release.yml`) does not run tests, so regressions are only caught locally via `scripts/precheck.sh`.
- Recommendation:
  - Prioritize tests for `repository`, `clipboard`, and `config` modules.
  - Add a CI workflow that runs `cargo test` on every PR.
  - Add concurrent read/write tests for the repository.

### 2. `AppTheme::System` triggers a panic in `set_app_theme`

**Priority:** Critical

- Relevant code:
  - `src/gui/app.rs` — `set_app_theme()` matches on `app_theme.get_theme()`, which resolves `System` to `Light` or `Dark` via `dark-light`. However, the final `match` arm is `AppTheme::System => todo!()`, which is unreachable through the normal path but can panic if `set_app_theme` is called with a raw `AppTheme::System` value that was not pre-resolved.
- Why it matters:
  - The `todo!()` is technically defensive dead code today because `get_theme()` always resolves `System`. However it is a latent panic risk that will trigger if any future caller passes `AppTheme::System` directly.
- Recommendation:
  - Replace `AppTheme::System => todo!()` with a recursive call to `get_theme()` or an `unreachable!()` with an explanatory comment.

### 3. `ContentType::FilePath` confirm flow is incomplete

**Priority:** High

- Relevant code:
  - `src/gui/board/` — confirm action for file-path records
  - `src/repository/repo.rs`
- Why it matters:
  - If a file-path record reaches the confirm flow, it can panic.
  - This was already documented in `doc/Optimization-Assessment-2026-03-30.md`.
- Recommendation:
  - Implement clipboard write behavior for file paths, or fail gracefully with a user-visible warning.

### 4. Errors are silently discarded in clipboard I/O

**Priority:** High

- Relevant code:
  - `src/clipboard/writer.rs` — `let _ = ctx.set_text(text);`
  - `src/clipboard/listener.rs` — `let _ = self.tx.send_blocking(...);`
  - `src/app.rs` — `let _ = notify_tx.send(record).await;`
- Why it matters:
  - Clipboard write failures are invisible to the user.
  - Channel send failures mean records are silently lost.
- Recommendation:
  - Replace `let _ =` with `if let Err(e) = ... { tracing::warn!(...) }` at minimum.

### 5. Duplicated lock-poisoning recovery pattern

**Priority:** Medium

The following pattern appears in 5+ locations across `src/app.rs` and `src/clipboard/listener.rs`:

```rust
let mut guard = match shared_records.lock() {
    Ok(g) => g,
    Err(poisoned) => poisoned.into_inner(),
};
```

- Recommendation:
  - Extract a helper function in `src/utils/`:
    ```rust
    pub fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<T> {
        mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
    ```

### 6. Hardcoded magic values are scattered

**Priority:** Medium

| Location | Value | Description |
|----------|-------|-------------|
| `src/gui/app.rs` | `px(400.), px(600.)` | Window dimensions |
| `src/gui/app.rs` | `0x002d2d2d`, `0x00ffffff`, etc. | Theme colors |
| `src/gui/paste.rs` | `Duration::from_millis(50)` | Paste delay |
| `src/gui/x11.rs` | `Duration::from_millis(10)` | X11 poll interval |
| `src/updater/checker.rs` | `"--connect-timeout", "15"` | HTTP timeout |
| `src/updater/downloader.rs` | `"--connect-timeout", "30"` | HTTP timeout (inconsistent) |
| `src/config/settings.rs` | `DEFAULT_MAX_HISTORY_RECORDS = 100` | Default limits |

- Recommendation:
  - Move defaults and constants to `src/constants.rs`.
  - Unify HTTP timeout values between checker and downloader.

### 7. Duplicated thread event forwarder pattern

**Priority:** Medium

`src/gui/hotkey.rs` (`spawn_hotkey_event_forwarder`) and `src/gui/tray.rs` (`spawn_tray_menu_event_forwarder`) share nearly identical spawn-and-forward logic.

- Recommendation:
  - Extract a generic `spawn_event_forwarder<T>` helper that accepts a receiver and a mapping closure.

### 8. Duplicated HTTP request logic in updater

**Priority:** Low

`src/updater/checker.rs` and `src/updater/downloader.rs` both construct `curl` commands with similar argument patterns.

- Recommendation:
  - Extract a shared `curl_command_builder` utility.

### 9. Time index upsert does not scale

**Priority:** Low (at current defaults)

- Relevant code:
  - `src/repository/time_index.rs` — `TimeIndex::upsert`, `TimeIndex::remove_by_id`
- Why it matters:
  - Stale entry removal scans the entire index.
  - Acceptable at the default `max_storage_records`, but degrades at higher values.
- Recommendation:
  - Add an auxiliary `id → timestamp` index for O(1) stale-entry removal.
  - Already documented in `doc/Optimization-Assessment-2026-03-30.md`.

### 10. Release profile tradeoff is untested

**Priority:** Low

- Relevant code:
  - `Cargo.toml` — `opt-level = "z"`, `lto = "fat"`
- Why it matters:
  - Optimizes aggressively for binary size, potentially at the cost of startup time and runtime responsiveness.
- Recommendation:
  - Benchmark `opt-level = "s"` and `opt-level = 3` against the current profile.
  - Already documented in `doc/Optimization-Assessment-2026-03-30.md`.

### 11. `RopyBoard` is a god struct with 30+ fields

**Priority:** Medium

- Relevant code:
  - `src/gui/board/mod.rs` — `RopyBoard` struct
- Why it matters:
  - `RopyBoard` manages UI state, settings editing, hotkey recording, update checking, record filtering, and repository operations in a single struct with 30+ fields.
  - This increases cognitive load and makes targeted refactors risky.
- Recommendation:
  - Extract cohesive subsets into dedicated structs:
    - `SettingsEditor` — `hotkey_recording`, `hotkey_manual_editing`, `pending_hotkey`, `hotkey_before_recording`, `settings_*_input`, `selected_theme`, `selected_language`, `language_select`, `autostart_enabled`, `auto_check_enabled`, `hover_preview_enabled`
    - `UpdateManager` — `update_status`
    - Keep `RopyBoard` focused on core board state: records, filtering, selection, and layout

### 12. `Mutex<Vec<ClipboardRecord>>` on UI thread — `RwLock` is more appropriate

**Priority:** Medium

- Relevant code:
  - `src/gui/board/mod.rs` — `records: Arc<Mutex<Vec<ClipboardRecord>>>`
  - `get_filtered_record_indices()` acquires the Mutex lock twice (once for filtering, once for sorting)
- Why it matters:
  - Records are read-heavy (every render cycle) and write-rare (only on clipboard events).
  - `Mutex` blocks a UI read if a background writer holds the lock. `RwLock` allows concurrent readers.
  - The double-lock pattern in `get_filtered_record_indices()` is wasteful; filtering and sorting should happen in a single lock scope.
- Recommendation:
  - Switch to `RwLock<Vec<ClipboardRecord>>` for the shared record cache.
  - Merge the two lock acquisitions in `get_filtered_record_indices()` into one.

### 13. Unsafe blocks lack `// SAFETY:` comments

**Priority:** Medium

- Relevant code:
  - `src/gui/utils.rs` — 5 `unsafe` blocks calling Windows API (`ShowWindow`, `SetForegroundWindow`, `SetWindowPos`, `ReleaseCapture`, `PostMessageA`) and macOS ObjC (`setActivationPolicy`)
  - `src/utils/single_instance.rs` — 1 `unsafe` block calling `CreateMutexW`, `GetLastError`, `FindWindowW`, `ShowWindow`, `SetForegroundWindow`
- Why it matters:
  - Clippy's `undocumented_unsafe_blocks` lint is not enabled, so these pass silently.
  - Reviewing correctness of `unsafe` code is harder without documented invariants.
- Recommendation:
  - Add `// SAFETY:` comments explaining why each `unsafe` block is sound.
  - Consider enabling `clippy::undocumented_unsafe_blocks = "warn"`.

### 14. `Language::display_name()` re-parses TOML on every call

**Priority:** Low

- Relevant code:
  - `src/i18n/language.rs` — `display_name()` reads from `LocaleAssets`, converts to UTF-8, and parses the full TOML file each time it is called.
- Why it matters:
  - Called during language dropdown rendering (once per language per render cycle).
  - Acceptable today but wasteful for embedded data that never changes at runtime.
- Recommendation:
  - Cache the parsed display names in a `OnceLock<HashMap<String, String>>` at first access.

### 15. Image filenames use nanosecond timestamps — collision risk

**Priority:** Low

- Relevant code:
  - `src/clipboard/utils.rs` — `let id = now.timestamp_nanos_opt().unwrap_or(0) as u64;`
- Why it matters:
  - Rapid consecutive clipboard image events can produce identical nanosecond timestamps, causing filename collisions.
  - The repository uses content hash as the record key, but the file system path is timestamp-based, creating an inconsistency.
- Recommendation:
  - Use the content hash (already computed by the listener) as the image filename to align with the repository key strategy.

### 16. Thumbnail path construction is fragile string manipulation

**Priority:** Low

- Relevant code:
  - `src/clipboard/utils.rs` — constructs `{id}_thumb.png` alongside `{id}.png`
  - `src/gui/board/records_list.rs` — `render_image_record()` derives thumbnail path by extracting `file_stem` and appending `_thumb.png`
- Why it matters:
  - The path derivation logic is duplicated and relies on filename conventions rather than a shared function.
  - If the naming convention changes, both locations must be updated in sync.
- Recommendation:
  - Extract a single `thumb_path_for(original: &Path) -> PathBuf` function.

### 17. Log files accumulate without rotation limits

**Priority:** Low

- Relevant code:
  - `src/utils/logging.rs` — `tracing_appender::rolling::daily(&log_dir, "ropy.jsonl")`
- Why it matters:
  - `tracing-appender`'s `daily` rolling creates a new file every day but never deletes old files.
  - On long-running installations, log files accumulate indefinitely.
- Recommendation:
  - Use `tracing_appender::rolling::RollingFileAppender::builder()` with `.max_log_files(N)` (available since `tracing-appender` 0.2.3) to cap retained log files.

### 18. `sled` is no longer actively maintained

**Priority:** Low (long-term)

- Relevant code:
  - `Cargo.toml` — `sled = "0.34.7"`
- Why it matters:
  - The `sled` author has stated the project is no longer actively maintained. No new releases since 2021.
  - Security patches and compatibility fixes will not be forthcoming.
- Recommendation:
  - Monitor for a suitable replacement (`redb`, `sqlite` via `rusqlite`, or `fjall`).
  - No immediate action needed — the current version works correctly.

## Engineering and CI Gaps

### 1. No test step in CI

The `release.yml` workflow only builds and publishes. There is no PR-level workflow that runs `cargo test`, `cargo clippy`, or `cargo fmt --check`.

- Recommendation:
  - Add a `ci.yml` workflow triggered on pull requests.

### 2. Missing documentation comments

Some modules have good doc comments (`updater/checker.rs`, `utils/hash.rs`), but many public functions lack documentation entirely.

- Recommendation:
  - Add `#![warn(missing_docs)]` to enforce documentation on public APIs incrementally.

## Positive Observations

### 1. No `unwrap()` or `expect()` in application code

Clippy rules enforce `unwrap_used = "warn"` and `expect_used = "warn"`, and the source code complies fully. This is excellent practice.

### 2. Unsafe code is minimal and justified

Only 6 `unsafe` blocks exist, all in platform-specific FFI code (`src/gui/utils.rs`, `src/utils/single_instance.rs`). No unsafe code in business logic. (Note: these blocks currently lack `// SAFETY:` comments — see Finding #13.)

### 3. Error types are well-defined

All modules use `thiserror` for error definitions, following the project convention in `AGENTS.md`.

### 4. Architecture boundaries are clean

The orchestration layer (`src/app.rs`) is well-separated from GUI rendering and repository internals, making targeted refactors low-risk.

### 5. Engineering toolchain is mature

- `scripts/precheck.sh` runs fmt, check, clippy, test, i18n check, and icon check.
- `scripts/record_build_size.sh` tracks binary size over time.
- `scripts/memory_profile.sh` provides macOS memory profiling.
- `scripts/check_i18n.py` validates translation key consistency.

### 6. Strict Clippy configuration

The project enables `clippy::pedantic` and `clippy::nursery` lints, which catches many subtle issues at compile time.

## Suggested Implementation Order

1. Add a CI workflow (`ci.yml`) that runs tests and clippy on PRs.
2. Add unit tests for `repository`, `clipboard`, and `config` modules.
3. Remove the `todo!()` in `set_app_theme` (replace with `unreachable!()` or resolve recursively).
4. Implement or gracefully handle `ContentType::FilePath` in the confirm flow.
5. Replace silent error discards with logged warnings.
6. Extract the lock-poisoning helper.
7. Switch `records` from `Mutex` to `RwLock` and merge the double-lock in `get_filtered_record_indices`.
8. Consolidate magic values into `src/constants.rs`.
9. Add `// SAFETY:` comments to all `unsafe` blocks.
10. Add `max_log_files` to the daily rolling appender.
11. Extract the event forwarder and curl builder helpers.
12. Begin decomposing `RopyBoard` into smaller structs.
13. Cache `Language::display_name()` results.
14. Use content hash for image filenames; extract `thumb_path_for()` helper.
15. Optimize time index upsert if storage limits are raised.
16. Benchmark release profile alternatives.
17. Evaluate `sled` replacement when a mature alternative stabilizes.

## Notes

- This assessment was performed as a read-only review.
- Findings from `doc/Optimization-Assessment-2026-03-30.md` are referenced where applicable to avoid duplication.

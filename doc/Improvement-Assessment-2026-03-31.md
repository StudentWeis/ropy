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

### 6. Hardcoded magic values are scattered

**Priority:** Medium

| Location | Value | Description |
|----------|-------|-------------|
| `src/gui/app.rs` | `px(400.), px(600.)` | Window dimensions |
| `src/gui/paste.rs` | `Duration::from_millis(50)` | Paste delay |
| `src/gui/x11.rs` | `Duration::from_millis(10)` | X11 poll interval |
| `src/updater/checker.rs` | `"--connect-timeout", "15"` | HTTP timeout |
| `src/updater/downloader.rs` | `"--connect-timeout", "30"` | HTTP timeout (inconsistent) |
| `src/config/settings.rs` | `DEFAULT_MAX_HISTORY_RECORDS = 100` | Default limits |

- Recommendation:
  - Move defaults and constants to `src/constants.rs`.
  - Unify HTTP timeout values between checker and downloader.

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

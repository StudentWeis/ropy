**Improvement Assessment (2026-03-31)**

This document records a comprehensive improvement review of the current Ropy codebase, covering code quality, test coverage, engineering practices, and architectural concerns.

## Priority Findings

- RFT 支持
- redb 支持

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

## Engineering and CI Gaps

### 1. No test step in CI

The `release.yml` workflow only builds and publishes. There is no PR-level workflow that runs `cargo test`, `cargo clippy`, or `cargo fmt --check`.

- Recommendation:
  - Add a `ci.yml` workflow triggered on pull requests.

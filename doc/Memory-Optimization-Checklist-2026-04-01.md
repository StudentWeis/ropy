# Memory Optimization Checklist (2026-04-01)

This checklist ranks the most promising memory optimizations for `ropy` based on the April 1, 2026 investigation.

The ranking prioritizes what is most likely to reduce the memory jump observed when the board window is opened on macOS. Secondary effects on resting footprint are also noted.

## How To Read This List

- Expected impact on visible peak:
  - High: likely to reduce the Activity Monitor spike in a meaningful way
  - Medium: likely to help, but probably not the main lever
  - Low: useful cleanup, but unlikely to move the big number much
- Expected impact on resting footprint:
  - High: likely to reduce hidden or background memory noticeably
  - Medium: measurable, but not dramatic
  - Low: probably small

## 1. Make Image Rendering More Aggressively On-Demand

Expected impact on visible peak: High

Expected impact on resting footprint: Low to Medium

Why this ranks first:

- The whole-process gap is much larger than the malloc-heap gap, which strongly suggests native rendering resources.
- Image rendering is the most plausible UI feature that can amplify native texture, `IOSurface`, and graphics memory on top of a relatively small Rust heap.
- The current list renders image rows with `img(...)` directly, and image tooltips also create image views on demand.

Relevant code:

- `src/gui/board/records_list.rs`
- `src/gui/board/preview.rs`

Most promising sub-actions:

- On macOS, default `hover_preview_enabled` to `false`.
- For image rows, render a lightweight placeholder or metadata row by default instead of always rendering a thumbnail.
- Only create the actual image view for:
  - the selected row with explicit preview enabled, or
  - a deliberate click action
- Keep using thumbnails where possible, but make sure list rows never request unnecessarily large image surfaces.

Why it is likely effective:

- It directly targets the feature most likely to cause native image and graphics allocations.
- It is also one of the few changes that can reduce visible-window memory without needing a deeper GPUI or AppKit refactor.

Main risk:

- Users may feel the board is less rich or less immediate when browsing image history.

Best validation:

- Compare image-heavy clipboard history before and after this change with `vmmap --summary` and Instruments `VM Tracker`.

## 2. Destroy And Recreate The Board Window Instead Of Only Hiding It

Expected impact on visible peak: Medium

Expected impact on resting footprint: High

Why this ranks second:

- Today, hiding the board uses `cx.hide()` and keeps the window object and UI tree alive.
- On macOS, closing and recreating an `NSWindow` is much more likely to release backing stores, `IOSurface`, and associated rendering resources than merely hiding it.

Relevant code:

- `src/gui/utils.rs`
- `src/gui/app.rs`
- `src/gui/board/mod.rs`

Most promising sub-actions:

- Replace the hide path with a true close-and-drop path for the board window.
- Recreate the window from the tray or hotkey when needed.
- Keep only the non-UI services alive in the background:
  - tray
  - clipboard monitoring
  - repository
  - hotkey listener

Why it is likely effective:

- It directly targets native window memory that `dhat` cannot explain.
- It should improve the hidden-state footprint more reliably than smaller Rust-side cleanups.

Main risk:

- More state-restoration complexity:
  - selected row
  - search state
  - preview state
  - focus behavior
- Window reopen latency may become more noticeable.

Best validation:

- Compare hidden-state `Physical footprint` before and after, immediately after the board is dismissed.

## 3. Lazily Create The Main Window On First Activation

Expected impact on visible peak: Low to Medium

Expected impact on resting footprint: High

Why this ranks third:

- Silent launch still constructs the full window tree at startup; it only sets `show: false`.
- If the user spends most of the time with `ropy` running in the background, there is no reason to pay any window construction cost until the first real activation.

Relevant code:

- `src/gui/app.rs`
- `src/app.rs`

Most promising sub-actions:

- Store enough shared app state to create the board window later.
- On startup:
  - initialize background services,
  - skip `create_window(...)`
- On first tray click or hotkey activation:
  - create the board window,
  - then show it

Why it is likely effective:

- It should reduce startup and idle memory in the common background-only case.
- It is simpler than full destroy-and-recreate on every hide, so it may be a good intermediate step.

Why it ranks below item 2:

- It helps hidden-state memory more than visible-window peak memory.
- Once the board is shown, most native window costs may still appear.

## 4. Reduce Or Replace The `sled` Baseline

Expected impact on visible peak: Low

Expected impact on resting footprint: Medium to High

Why this ranks fourth:

- The Rust heap profile shows a large fixed baseline associated with `sled`.
- That baseline appears even before the board is shown, so it is not the main explanation for the visible-window spike.
- Even so, cutting 15 to 25 MiB from the always-on background process would still be meaningful.

Relevant code:

- `src/repository/sled_backend.rs`
- `src/repository/redb_backend.rs`

Most promising sub-actions:

- Benchmark `redb` as the default backend on macOS.
- Compare startup memory and steady-state memory between:
  - `sled`
  - `redb`
- If staying on `sled`, investigate why the fixed large allocations are still present in the current build and whether any feature or configuration can remove them.

Why it is likely effective:

- It targets the clearest Rust-side baseline cost found in `dhat`.

Why it does not rank higher:

- It probably will not fix the "window opens and Activity Monitor spikes" symptom by itself.

Main risk:

- Storage migration and behavioral differences between backends.

## 5. Lazy-Initialize Settings-Only Controls And Secondary UI State

Expected impact on visible peak: Low

Expected impact on resting footprint: Medium

Why this ranks fifth:

- `RopyBoard::new` eagerly creates multiple controls that are only needed in the settings view:
  - settings hotkey input
  - max-history input
  - max-storage input
  - theme select
  - language select
- The investigation suggests these are not a major memory hotspot, but they are still eager work done on every launch.

Relevant code:

- `src/gui/board/mod.rs`
- `src/gui/panel/settings.rs`

Most promising sub-actions:

- Create settings inputs and selects only when the settings panel is first opened.
- Store them in an optional settings-state struct instead of the root board struct.

Why it is still worth doing:

- It is a clean architectural improvement.
- It reduces always-on UI state.

Why it ranks lower:

- The measured data suggests this is a small win, not a primary lever.

## 6. Move Update Checking Off The Startup Critical Path

Expected impact on visible peak: Low

Expected impact on resting footprint: Low

Why this ranks sixth:

- Auto-update checking runs soon after launch.
- The memory investigation did not show it as a major contributor.

Relevant code:

- `src/app.rs`
- `src/gui/board/updater_ui.rs`

Most promising sub-actions:

- Delay auto-check until after the board has been shown once.
- Or only check from the tray or About panel.

Why it ranks low:

- Good cleanup, but unlikely to materially change the numbers you care about.

## 7. Treat GPUI And macOS Window Tuning As Experiments, Not First Moves

Expected impact on visible peak: Unknown

Expected impact on resting footprint: Unknown

Why this ranks last:

- The investigation strongly suggests native window and graphics memory are involved.
- But without a focused Instruments session, it is still hard to say which exact GPUI or AppKit lever is responsible.
- The current default window size is already modest at 400 x 600, so simple size changes are unlikely to be the main answer.

Relevant code:

- `src/gui/constants.rs`
- `src/gui/app.rs`

Possible experiments:

- Look for GPUI window options that reduce native backing overhead.
- Evaluate whether popup-window configuration is contributing to the observed spike.
- Compare a simplified, text-only board window against the full board UI.

Why it ranks low for immediate work:

- These experiments are lower-confidence than the more direct changes above.

## Recommended Execution Order

If you want the best balance between likely payoff and engineering risk, this is the order I would actually try:

1. Make image previews more conservative on macOS.
2. Add a code path that destroys and recreates the board window.
3. If needed, make window creation lazy at first activation.
4. Benchmark `redb` against `sled`.

## One-Sentence Summary

The best bets are the changes that reduce native rendering work when the board is shown, and only after that should we spend serious effort on shrinking Rust-side baseline memory.

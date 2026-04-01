# Memory Investigation (2026-04-01)

This document records the findings from the macOS memory investigation performed on April 1, 2026.

## Goal

Explain why `ropy` appears to use about 50 MB at rest but can spike to about 150 MB when the UI is opened in Activity Monitor.

## Local Artifacts From This Investigation

- Rust heap profile:
  - `target/dhat-heap.compat.json`
- Whole-process macOS snapshots:
  - `target/memory_profile_20260401_231425/`
  - `target/memory_profile_20260401_231546/`

These artifact directories were generated locally during the investigation and are useful for re-checking the numbers below.

## Short Conclusion

The large jump seen in Activity Monitor is most likely real, but it does not appear to be primarily caused by Rust heap growth.

- `dhat` indicates that Rust-managed live heap at peak is only about 30.7 MiB.
- The main Rust heap baseline comes from `sled` plus a smaller amount of `gpui` and `gpui-component` state.
- Whole-process macOS snapshots show that the big jump when the window is shown is much more consistent with native window and rendering memory such as `IOSurface`, `IOAccelerator`, and other AppKit/CoreAnimation/Metal-related costs.

## Why This Investigation Needed Two Tools

`dhat` answers "what is on the Rust heap?"

- It is good for Rust allocation call stacks.
- It does not fully account for AppKit, CoreAnimation, Metal, `IOSurface`, or window backing stores.

`vmmap` and `heap` answer "what is the whole process using right now?"

- They include native UI and graphics allocations.
- They are the right tools for understanding the number shown by Activity Monitor.

## Relevant Runtime Behavior In `ropy`

Two implementation details matter for interpreting the results.

- The app creates the main window eagerly at startup, even in silent mode. Silent mode only sets `show: false`.
- The board hides itself on focus loss when settings are not open.

Relevant code:

- `src/gui/app.rs`
- `src/gui/board/mod.rs`

This means a memory spike can happen during or shortly after window activation, and the current footprint can later fall back once the board hides again.

## DHAT Summary

The Rust heap profile supports the idea that Rust is not the main source of the 150 MB peak.

- Sum of live bytes at global peak across profile points: about 30.7 MiB
- Main categories at peak:
  - `sled`: about 27 to 28 MiB
  - `gpui`: about 2.7 MiB
  - `gpui-component`: about 0.6 MiB

Additional observations:

- A large Rust baseline comes from forty-two fixed 512 KiB allocations associated with `sled` internals, totaling about 21.5 MiB.
- `sled` page table allocations contribute another roughly 6.0 MiB.
- `gpui` layout and arena state account for a few MiB, but not tens of MiB.
- Loaded clipboard records themselves did not show up as a major retained-memory hotspot in this profile.

## Whole-Process Sampling Summary

### Sample A: Hidden or Resting State

Artifact directory:

- `target/memory_profile_20260401_231425/`

Key numbers:

- `vmmap --summary`
  - Physical footprint: 52.9M
  - Physical footprint peak: 145.0M
- `heap`
  - Physical footprint: 52.8M
  - All malloc zones: 39,988,512 bytes

Notable regions:

- `IOAccelerator (graphics)`: 10.5M virtual, 4256K resident
- `IOSurface`: 3824K resident
- `MALLOC_SMALL`: 53.5M virtual, 6192K resident

### Sample B: Window Shown

Artifact directory:

- `target/memory_profile_20260401_231546/`

Key numbers captured while the visible build was active:

- `heap`
  - Physical footprint: 161.1M
  - Physical footprint peak: 161.1M
  - All malloc zones: 41,106,544 bytes

Important comparison against Sample A:

- Whole-process physical footprint increased by roughly 108 MiB.
- Malloc zone bytes increased by only about 1.1 MiB.

That is the strongest evidence from tonight's run that the jump is not mainly normal heap growth.

At nearby moments from `vmmap --summary`, the current footprint was closer to 61 to 65 MB while the process peak remained above 153 MB. This is consistent with the board auto-hiding after focus changes and releasing part of the native rendering footprint while still preserving the high peak value.

Notable regions when the window had been shown:

- `IOSurface` grew from about 3.8M to about 11.2M
- `IOAccelerator (graphics)` grew in resident usage and region count
- Read-only library residency also increased significantly
- `__TEXT` residency increased significantly

## Interpretation

The current best explanation is:

1. `ropy` starts with a Rust heap baseline dominated by `sled` and a small amount of GPUI state.
2. When the board window is shown, macOS allocates additional native window and graphics resources.
3. Activity Monitor reflects that whole-process peak, so the app can briefly look like a 150 MB process even though the Rust heap remains much smaller.
4. When the board hides again, current memory may fall back, but peak numbers remain high.

## What Tonight's Data Does Not Suggest

The data does not suggest that:

- the clipboard record list alone is consuming 100 MB of Rust heap,
- the lazy list is rendering all history items into large retained Rust structures,
- the configured `sled` page cache is the direct cause of the 150 MB UI-open spike.

For reference, the repository already caps the `sled` cache to 8 MiB in `src/repository/sled_backend.rs`.

## Caveats

- `ps` RSS understated the issue and was much less useful than `vmmap` and `heap`.
- Because the board hides on focus loss, launching from Terminal is not a perfect reproduction of a stable visible window.
- The exact current value varies depending on whether the board is frontmost at the instant of sampling.
- The peak values were more informative than single current-value snapshots.

## Recommended Follow-Ups

### Highest-value validation

- Keep the board visible and frontmost while capturing a fresh Instruments session with:
  - `Allocations`
  - `VM Tracker`
- Verify whether `IOSurface`, CoreAnimation, or Metal-backed resources dominate the visible-window peak.

### Likely code experiment

- Try lazy window creation instead of eager window creation at startup.
- Today, silent launch still constructs the window tree and only suppresses initial display.
- Deferring actual window creation until first activation may reduce startup-time or first-show peak behavior.

### Secondary checks

- Compare memory behavior with and without image-heavy clipboard history.
- Compare memory behavior with hover preview disabled.
- Re-check whether any GPUI window configuration can reduce native backing-store overhead on macOS.

## Final Takeaway

Based on the April 1, 2026 investigation, the "50 MB at rest, 150 MB when opened" report is plausible and is most likely driven by native macOS UI and graphics memory rather than a large Rust heap retention bug inside `ropy`.

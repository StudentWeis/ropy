# Windows RDP Popup DPI Restore

## Background

On Windows, Ropy keeps the main window as a popup. After disconnecting a Remote Desktop session, reopening the popup could leave it at an unexpectedly large size.

This document records the analysis and the mitigation added in April 2026.

## Symptom

- Reconnect or return from a Remote Desktop session.
- Trigger the popup again through the global hotkey or tray action.
- The popup appears much larger than the normal 400x550 logical size.

## Root Cause

The activation path previously relied on a fixed logical resize before restoring the popup:

- The board activation flow always resized the window back to the default logical size.
- The Windows activation flow then restored the native window with `ShowWindow(SW_RESTORE)`.
- GPUI's Windows backend can see monitor/display changes during RDP disconnects, but in this specific path the cached DPI state may be stale when the popup is shown again.

When the cached scale factor still reflects the remote session, resizing to the default logical size can map to the wrong physical size on the local monitor.

## Constraints

- The popup window type must remain `WindowKind::PopUp`.
- The fix should happen in the application layer instead of changing window kind semantics.
- The activation path should remain deterministic and low-risk.

## Implementation

The fix adds a Windows-specific geometry reset before restoring the popup.

### New activation helper

`src/gui/utils.rs` now exposes `reset_window_geometry_for_activation`.

On Windows it:

1. Gets the current `HWND` from GPUI.
2. Resolves the current monitor with `MonitorFromWindow`.
3. Reads the monitor work area with `GetMonitorInfoW`.
4. Reads the effective monitor DPI with `GetDpiForMonitor`.
5. Falls back to `GetDpiForWindow` if monitor DPI lookup fails.
6. Measures current non-client frame extents from the live window rectangle and client rectangle.
7. Recalculates the physical window rectangle from the default logical popup size.
8. Centers that rectangle inside the current monitor work area.
9. Applies the recalculated native geometry with `SetWindowPos` before restore/foreground activation.

On non-Windows platforms it falls back to the existing logical `window.resize(...)` behavior.

### Activation flow change

The board activation path now calls `reset_window_geometry_for_activation(window, default_window_size())` before `active_window(...)`.

This keeps the popup behavior intact while ensuring the native geometry is refreshed from the current monitor state first.

## Why This Fix

This approach was chosen because it:

- Preserves popup behavior.
- Avoids depending on GPUI's cached DPI state during a fragile RDP transition.
- Recomputes geometry from the actual current monitor instead of the last known session state.
- Limits the change to the activation path where the issue is observed.

## Validation

The following checks were used during implementation:

- `cargo check`
- `cargo test test_calculate_activation_window_geometry -- --nocapture`

Unit tests were added for the pure geometry calculation used by the Windows helper.

## Known Limits

- The geometry math is covered by tests, but runtime behavior still needs validation on a real Windows machine across actual RDP connect/disconnect cycles.
- The current mitigation is application-side. If future issues show GPUI still exposing stale DPI state in other paths, upstream investigation may still be needed.

## Related Files

- `src/gui/utils.rs`
- `src/gui/board/actions.rs`
- `src/gui/mod.rs`
- `Cargo.toml`

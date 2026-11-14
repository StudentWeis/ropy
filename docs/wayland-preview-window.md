# Native preview popup on Wayland

Status: implemented in the working tree for [Issue #346](https://github.com/StudentWeis/ropy/issues/346), November 2026; not released. The implementation uses gpui-kit 0.9.1 and the locked gpui-pre 0.3.12 backend. Native acceptance is separate from the headless regression tests below.

The hold-key and hover preview of a clipboard record opens a second native window. Wayland now uses a non-grabbing `WindowKind::AnchoredPopup` parented to the history window. X11, macOS and Windows keep `WindowKind::PopUp` and the existing absolute-bounds placement. This page records the original problem, the placement decision and the remaining native acceptance checks.

## Original symptom

Before this change, on a tiling Wayland compositor such as Hyprland, opening the preview:

- makes the preview a tiled window, which resizes and rearranges the windows already on the workspace;
- places the preview wherever the compositor decides, not beside the history popup;
- may move keyboard focus from the history window to the preview, depending on compositor policy.

Focus transfer alone does not prove that hold preview cannot close. The current preview handles the preview key's release itself (`PreviewWindow::render` in [preview_window.rs](../src/gui/board/preview_window.rs)). The board also defers its focus check while a preview exists and keeps both windows open if either remains active (`observe_activation` in [board.rs](../src/gui/board.rs), `defer_preview_focus_check` in `preview_window.rs`). Any reported stuck preview needs a reproduction that shows why these paths did not handle it.

## Original root cause

The preview window was created in [preview_window.rs](../src/gui/board/preview_window.rs) with `WindowKind::PopUp`, a requested size and absolute bounds, and no `app_id`. Each platform backend interprets that kind differently:

| Platform | `WindowKind::PopUp` | Result |
| --- | --- | --- |
| X11 | `override_redirect = true` (`gpui-pre-linux`, `src/linux/x11/window.rs`) | Unmanaged window, application-chosen position, never tiled |
| macOS | Floating panel level | Uses the main popup's level; Ropy requests `focus: false` at creation |
| Wayland | Falls through to a plain `xdg_toplevel` (`gpui-pre-linux`, `src/linux/wayland/window.rs`) | Ordinary application window managed by the compositor |

The Wayland backend only sets `xdg_toplevel.app_id` when `WindowOptions::app_id` is `Some`, so the old preview window had no application ID that a compositor rule could match. The main popup sets `com.ropy.app` and installs a Hyprland floating rule before mapping ([app.rs](../src/gui/app.rs), [hyprland.rs](../src/linux/hyprland.rs)); the preview had no equivalent.

`xdg_toplevel` has no field that expresses "float, unmanaged, position at this screen coordinate". That limit, not a missing Ropy setting, caused the preview to be tiled on Wayland.

## Constraints the fix must respect

- Opening the preview should preserve keyboard focus in the history window. Releasing the preview key must close a hold preview, and clicking preview controls must not hide the history window. Preserve the existing key-release handlers and deferred focus check; do not assume all focus transfers hide the board. Auto-hide also depends on pin and dialog state.
- The preview must stay above the history popup and be dismissed with it.
- A Wayland client cannot position a toplevel. `window.bounds()` reports an origin of `(0, 0)` on Wayland; only the size is real. Wayland placement must use parent-local coordinates, not the screen-coordinate arithmetic in `preview_bounds`.
- X11, macOS and Windows behavior must not change.

## Implementation

`preview_placement` creates `WindowKind::AnchoredPopup` on Wayland. The actual native window handle selects the backend, so an X11 or headless window running inside a Wayland session keeps the existing path.

`PopupOptions` names the parent window explicitly and places the preview relative to an anchor rectangle in that window's coordinate space, with an anchor point, gravity, offset and constraint adjustment. Set `parent` to the history window handle and `grab: false` to avoid requesting an explicit input grab. GPUI registers this popup as a non-blocking child. This establishes the request sent to the compositor, not a verified cross-compositor guarantee about focus after opening, clicking or dragging.

The [xdg-shell protocol](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/stable/xdg-shell/xdg-shell.xml) specifies keyboard focus for an explicitly grabbing popup; that rule alone does not establish that a non-grabbing popup can never receive keyboard input. Verify input routing on each tested compositor. GPUI already handles `xdg_popup` resize by re-running the positioner and handles `popup_done`. Preserve Ropy's explicit preview cleanup when hiding or closing the history window rather than relying only on compositor dismissal.

The placement policy preserves popup-edge docking rather than covering neighboring records:

- `preview_anchor` retains the existing vertical alignment, including viewport origin and scroll offset for grid cards. `preview_placement` converts that position to parent-local coordinates and clamps a one-pixel-high anchor rectangle inside the parent.
- List records prefer the right history-window edge. Left-half grid records prefer the left edge; right-half records prefer the right edge. The popup grows downward and outward with the existing 10-pixel gap.
- Slide, flip and resize constraint adjustment are enabled on both axes. The compositor handles work-area fitting; manual left/right space arithmetic remains only on the other platforms.
- Size comes from the existing `preview_target_size`; the popup's requested origin is ignored.

Platform matrix:

| Platform | Window kind | Placement |
| --- | --- | --- |
| Wayland | `AnchoredPopup` | Anchor rectangle in the parent window, adjusted by the compositor |
| X11 | `PopUp` | Current absolute-bounds math, unchanged |
| macOS, Windows | `PopUp` | Current absolute-bounds math, unchanged |

GPUI does not expose an update of a mapped popup's anchor rectangle. Hover switching compares the stored popup options and recreates the preview when the anchor changes. A size-only change uses the existing resize path. The Wayland path does not treat the requested or reported origin as the compositor's actual position. Other platforms still recreate when their calculated absolute origin changes.

## Alternatives

### Hyprland window rule

Give the preview its own `app_id` and extend `PopupRules` (`src/linux/hyprland.rs`) with a `float = true` rule. A local probe on Hyprland 0.56.2 showed that the Lua `hl.window_rule` API accepts boolean rule fields including `no_initial_focus`, and rejects unknown fields.

Rejected as the primary fix because:

- Position is still unavailable. The rule can center the window or apply a `move` vector that depends on the parent popup's real screen position, which requires an extra IPC query and races with mapping.
- `HL.WindowRule` exposes only `is_enabled` and `set_enabled`; a rule carrying a per-preview size or position has no documented update path.
- The fix covers Hyprland only. Sway, Niri, GNOME and KDE Wayland sessions keep the current behavior.

A floating toplevel can take focus on click, but this is not by itself a reason to reject the alternative: Ropy's deferred focus check already allows focus inside the preview. Any remaining focus failure requires native evidence.

### Layer-shell overlay

`WindowKind::LayerShell` offers layer placement through anchors and margins and can be configured without keyboard interactivity. Rejected because GNOME does not support layer-shell, current GPUI layer surfaces do not provide the same resize and reposition behavior, and the overlay layer changes the window's meaning rather than its placement.

## Failure and recovery

- If `open_window` returns a synchronous error, the existing path logs it. Hold mode also shows `preview_open_failed`; hover mode does not show a notification. No preview session is installed on this path, and the history window remains available.
- Normal compositor dismissal arrives asynchronously as `popup_done`. GPUI handles this event as a close request; the board's window-closed subscription is intended to clear the matching session. Verify this end-to-end, including reopening after dismissal.
- A Wayland protocol error is not an ordinary `open_window` error or normal dismissal. It can terminate the client connection, so the synchronous error handler cannot guarantee recovery. Use a valid mapped parent and valid anchor geometry, and check for protocol errors during native acceptance.
- The compositor may shrink the popup when the requested size does not fit the output. Preview content must tolerate a smaller area; the image viewport already fits to the canvas.
- Hiding, closing or recreating the history window must remove its preview and clear the session. Verify these paths separately from `popup_done`; do not assume that unmapping the parent alone performs all application cleanup.

## Verification on November 14, 2026

`cargo test --locked -p ropy preview -- --nocapture` passed all 35 matching tests. `./scripts/precheck.sh --check` passed formatting, resources, 92 script tests, Clippy, workspace tests, the private-bus Linux tray regression and rustdoc.

A local Linux x86-64 debug build from baseline `8da0f11` plus this implementation was tested in an isolated Hyprland 0.56.2 session with a 1440×1080 headless output, XWayland disabled and `DISPLAY` unset. The executable SHA-256 was `b105864ff8b967d8d19d859192737c24ce7966d5f24a3ef926f52c5d7acae05e`. Disposable XDG directories, a private session bus and an isolated clipboard kept the test separate from the host data.

At verified output scales of 100% and 150%, two hold-preview cycles each reached popup configure and buffer attachment. Screenshots showed the preview beside the history window, with compositor sliding under constrained space at 150%. Protocol traces showed an explicitly parented `xdg_popup` without a grab. The preview added no managed toplevel, a tiled peer kept its geometry, the history window stayed active, and releasing Space destroyed the popup. Compositor-generated key events through Hyprland IPC exercised native client delivery; this was not a physical-keyboard test.

Escape closed only the preview. External focus loss while a hold preview was open closed both the unpinned history window and preview; reopening the history window succeeded. No Wayland protocol errors were observed.

Native hover switching, grid scrolling, image controls and dragging, pinned cleanup, asynchronous `popup_done`, a second monitor, and other compositors remain untested. Headless behavior tests cover several of these application paths, but do not replace native acceptance. This development-build check is not packaged-release certification.

## Acceptance

Unit tests:

- Placement tests cover parent-local edge anchors, left/right preference, clamping, non-grabbing parent options, compositor constraints and requested size. Reopen tests distinguish anchor changes from size-only updates and retain the non-Wayland bounds contract.
- Existing preview UI tests: the headless test platform ignores `WindowKind`, so they keep covering behavior rather than placement.

Manual checks on Hyprland with `WAYLAND_DISPLAY` set and `DISPLAY` unset:

1. Open the history window and hold the preview key. Compare `hyprctl clients` before and after: the history window remains `com.ropy.app`, and the preview adds no second managed toplevel. Other applications may still appear in the list.
2. Other windows on the workspace keep their tiling; nothing moves when the preview opens.
3. `hyprctl activewindow` stays on `com.ropy.app` while the preview is visible, and releasing the key closes the preview.
4. Hover preview switching between records follows the chosen placement policy without closing the history window. Check scroll offsets and records at viewport edges.
5. Clicking a preview control and dragging an image do not dismiss the popup.
6. Check Escape routing while the preview is open. Record whether it closes only the preview or also hides the history window; compare this with the chosen interaction policy.
7. Hide, close and reopen the history window with a preview open, both pinned and unpinned. Verify that no orphan preview or stale session remains. Exercise compositor dismissal where available and reopen the preview afterward.
8. Repeat at 100% and 150% output scale, on a second monitor, and near output edges with constrained space. Check for Wayland protocol errors.

`hyprctl activewindow` identifies the active managed window, not proof of keyboard delivery to a particular surface. Pair it with the key-release and Escape checks. Record the artifact, compositor version and reproduction steps for any focus failure.

Report other compositors as untested unless they were run, following [Testing](testing.md) and [release acceptance](release-acceptance.md).

## Decisions

- Keep popup-edge placement. Record-relative inward placement would cover other records and change the existing interaction.
- Change only the Wayland surface role. Keep placement state local to the preview module; no dependency upgrade or cross-platform popup migration is needed.
- Escape closes the preview first. The existing history-window hide action and preview key handler both implement this policy. Native routing with `grab: false` passed on the isolated Hyprland session above; other compositors still need acceptance.

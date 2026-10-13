# Local Wayland regression on macOS

Use a disposable Linux VM to test the native Wayland popup against a **native
Wayland editor**, with XWayland disabled. A successful X11-only clipboard test does not cover this boundary.
See [WAYLAND.md](WAYLAND.md) for the supported application behavior.

The native results below correspond to [PR #234](https://github.com/StudentWeis/ropy/pull/234)
(source commit `424c718`, merged as `6328331`). They establish the Sway regression
baseline, not certification of every Wayland desktop.

## Tested environment

The October 2026 regression used Lima 2.2.1 with Apple's Virtualization.framework:
ARM64 Ubuntu 26.04.1, 6 CPUs, 8 GiB RAM and a 60 GiB disk. The source worktree was
mounted read-only; source copies, Cargo output, configuration and clipboard history
stayed inside the guest. Sway 1.11 ran with software rendering. The earlier
resident-window refactor was tested with XWayland; native popup tests use
`xwayland disable` and remove `DISPLAY` from the application environment.

Install the normal Linux build dependencies from `.github/dist-build-setup.yml`,
plus `sway`, `wl-clipboard`, `grim`, `python3-gi`, `gir1.2-gtk-3.0`,
`python3-evdev`, `seatd`, `fonts-dejavu-core`, `libayatana-appindicator3-1` and
`at-spi2-core`. Use the Rust toolchain specified by the repository. Install
`xwayland` only when reproducing the historical XWayland comparison; it is not
required for the native scenario.

Build the baseline and candidate in separate source directories with a shared
guest target directory. Record each source revision, dirty diff and binary SHA-256.
Run `scripts/precheck.sh` before the GUI checks. When copying from macOS, use
`COPYFILE_DISABLE=1 tar --no-xattrs` to exclude AppleDouble `._*` files; otherwise
resource embedding can include those metadata files as application assets. Four Cargo jobs and disabled dev/test
debug information kept builds within this VM's memory budget.

## Desktop setup

Run Sway on a private session bus and private XDG directories. Use these environment
variables for a headless output with the VM's real virtual input devices:

```sh
export XDG_SESSION_TYPE=wayland
export WLR_BACKENDS=headless,libinput
export LIBSEAT_BACKEND=seatd
export WLR_RENDERER=pixman
export WLR_HEADLESS_OUTPUTS=1
export WLR_LIBINPUT_NO_DEVICES=1
export LIBGL_ALWAYS_SOFTWARE=1
unset DISPLAY WAYLAND_DISPLAY SWAYSOCK HYPRLAND_INSTANCE_SIGNATURE
dbus-run-session -- sway -c /path/to/private/sway.conf
```

The private `XDG_RUNTIME_DIR` must exist with mode 0700. A VM without a login seat
also needs a dedicated seatd process with `SEATD_VTBOUND=0` and access for the guest
user. Do not change the host desktop's seat or input services.

Configure a 1280×960 output, disable focus-follows-mouse, float the Ropy window and
bind `Ctrl+Shift+d` to the candidate executable with `--show`. The native
window has app ID `com.ropy.app`; see the floating rule in [WAYLAND.md](WAYLAND.md). Export the compositor
child environment into a private file so the editor, app and inspection commands
share the same display and session bus. Give each test an isolated `XDG_CONFIG_HOME`.
Disable autostart and automatic update checks in that test configuration.

Use a GTK TextView with `GDK_BACKEND=wayland`; assert that its display type is
`GdkWaylandDisplay`. Inspect text and active state from the editor itself. Inspect
Ropy's visibility and focus through `swaymsg -t get_tree`, and the selection through
`wl-paste`. Capture the output with `grim`.

For unattended key chords, inject evdev press/release events into the guest's
`Apple Inc. Virtual USB Keyboard`; always release modifiers in a `finally` block.
A virtual-keyboard-only Sway seat can have an incomplete keymap, producing false
copy/paste failures. Do not force window focus to turn a failed activation into a pass.

## Scenario and observed results

Repeat with `copy_to_clipboard` and `paste_immediately` confirmation modes:

1. Start Ropy and verify that it stays alive with no visible popup.
2. Type a unique marker in the native editor, select/copy it, then clear the editor.
3. Send the compositor shortcut; assert the popup is visible **and focused**,
   uses `xdg_shell`, and has app ID `com.ropy.app`.
4. Search for the marker with `/`, then confirm with Enter.
5. Assert the popup hides, editor focus returns and the clipboard equals the marker.
6. In copy mode, send Ctrl+V; in immediate mode, wait without sending another paste.
   Assert the editor receives the marker exactly once.
7. Repeat open/Escape twice, checking focus restoration each time.
8. Run a second process with `--show`; assert the retained instance opens and dismisses.
9. Copy a new unique marker while the popup is destroyed. Reopen, search and
   confirm it; assert the new marker reaches the editor exactly once.
10. In copy mode, pin the popup and change focus to the editor: the popup must
    remain visible. Reactivate, unpin and change focus: it must close. Immediate
    paste mode intentionally does not offer pinning. Test focus loss separately
    from activation; never force focus to rescue a failed shortcut.
11. Repeat with the headless output at scale 1.6; check the 400 × 550 logical
    content size and inspect the rendered search screenshot.

Keep the pinned-window activation policy explicit. Under Sway's default
`focus_on_window_activation urgent`, a valid `--show` token marks the existing
window urgent without refocusing it. Record that result separately. The complete
pin/reactivate/unpin scenario uses `focus_on_window_activation smart` on the
private desktop; the compositor binding must then restore focus naturally.

The earlier XWayland baseline/candidate × confirmation-mode runs passed before
the native change. Keep native results separately, including the application log,
assertion report, binary digest and history/search/paste screenshots. The native
scenario covers Sway compositor activation, focus return, hidden clipboard capture
and popup reconstruction. It does not certify a desktop portal implementation,
Hyprland registration, tray interaction, hardware rendering or x86-64 builds.
Headless GPUI tests separately cover tray settings/about routing after dismissal.
The native candidate passed both confirmation modes at 160% scaling with XWayland
disabled and `DISPLAY` unset: 26 copy-mode and 23 immediate-paste assertions.
The floating content remained 400 × 550 logical pixels (640 × 880 physical pixels).

## Portal contract fixture

`scripts/tests/wayland_portal_fixture.py` supplies a fake GlobalShortcuts service
on an isolated D-Bus session while the real Ropy window runs in Sway. Install
`python3-dbus` and `python3-gi`, then run each mode separately:

```sh
dbus-run-session -- python3 scripts/tests/wayland_portal_fixture.py \
  --binary /path/to/ropy \
  --environment /path/to/private/sway-session.env \
  --input-helper /path/to/evdev-chord-helper.py \
  --output /path/to/evidence/portal-accept \
  --mode accept
```

The environment file uses `KEY=value` lines exported by the compositor. The
fixture keeps its own private `DBUS_SESSION_BUS_ADDRESS`. The helper accepts a
single chord, including `Escape` and `alt+F4`; it runs with `sudo` only inside the
disposable guest to inject events into its virtual keyboard.

Repeat with `--mode deny`, `--mode pending` and `--mode startup`, using separate
output directories.
Accept mode verifies preferred-trigger encoding, unrelated session/shortcut
filtering, activation-token delivery in the actual Wayland protocol trace,
second-process token forwarding after dismissal, and explicit session close on
normal quit. Deny mode checks release of the private connection and continued
manual activation. Pending mode leaves authorization unanswered and checks that
normal quit cancels it and releases the connection. Each report records the
binary SHA-256. Startup mode checks that `ropy --show` preserves the first
surface and uses its startup token exactly once. Protocol traces contain only
fixture activation tokens.

All four fixture modes passed against the same native candidate binary.

This fixture is client-contract evidence, not validation of GNOME/KDE permission
UI, desktop shortcut persistence or focus approval for a real trusted token.

## Environment limits and cleanup

Hyprland 0.56.2 could not start in this VZ guest: there was no DRM GPU, and the
software-rendered parent lacked protocols required by Aquamarine's nested backend.
Classify that row as **infrastructure blocked**, not an application failure or pass.
On a working Hyprland Lua desktop, additionally run
`cargo test --bin ropy test_live_hyprland_activation_reload_and_cleanup -- --ignored`
and repeat the GUI scenario using Ropy's registered shortcut.

Stop the recorded app/editor/input/compositor processes after collecting evidence,
then stop the VM. Retain its disk for subsequent regression runs.

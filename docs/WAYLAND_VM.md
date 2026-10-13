# Local Wayland regression on macOS

Use a disposable Linux VM to test the XWayland popup against a **native Wayland
editor**. A successful X11-only clipboard test does not cover this boundary.
See [WAYLAND.md](WAYLAND.md) for the supported application behavior.

## Tested environment

The October 2026 regression used Lima 2.2.1 with Apple's Virtualization.framework:
ARM64 Ubuntu 26.04.1, 6 CPUs, 8 GiB RAM and a 60 GiB disk. The source worktree was
mounted read-only; source copies, Cargo output, configuration and clipboard history
stayed inside the guest. Sway 1.11 ran with XWayland and software rendering.

Install the normal Linux build dependencies from `.github/dist-build-setup.yml`,
plus `sway`, `xwayland`, `wl-clipboard`, `grim`, `python3-gi`, `gir1.2-gtk-3.0`,
`python3-evdev`, `seatd`, `fonts-dejavu-core`, `libayatana-appindicator3-1` and
`at-spi2-core`. Use the Rust toolchain specified by the repository.

Build the baseline and candidate in separate source directories with a shared
guest target directory. Record each source revision, dirty diff and binary SHA-256.
Run `scripts/precheck.sh` before the GUI checks. Four Cargo jobs and disabled dev/test
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
bind `Ctrl+Shift+d` to the candidate executable with `--show`. Export the compositor
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

For both the baseline and candidate, repeat with `copy_to_clipboard` and
`paste_immediately` confirmation modes:

1. Start Ropy and verify that it stays alive with no visible popup.
2. Type a unique marker in the native editor, select/copy it, then clear the editor.
3. Send the compositor shortcut; assert the popup is visible **and focused**.
4. Search for the marker with `/`, then confirm with Enter.
5. Assert the popup hides, editor focus returns and the clipboard equals the marker.
6. In copy mode, send Ctrl+V; in immediate mode, wait without sending another paste.
   Assert the editor receives the marker exactly once.
7. Repeat open/Escape twice, checking focus restoration each time.
8. Run a second process with `--show`; assert the resident popup opens and dismisses.

All four baseline/candidate × confirmation-mode runs passed. Keep the per-run
application log, assertion report, binary digest and history/search/paste screenshots.
This covers Sway's compositor shortcut forwarding; it does not prove automatic
Hyprland shortcut registration, tray behavior, hardware rendering or x86-64 builds.

Hyprland 0.56.2 could not start in this VZ guest: there was no DRM GPU, and the
software-rendered parent lacked protocols required by Aquamarine's nested backend.
Classify that row as **infrastructure blocked**, not an application failure or pass.
On a working Hyprland Lua desktop, additionally run
`cargo test --bin ropy test_live_hyprland_activation_reload_and_cleanup -- --ignored`
and repeat the GUI scenario using Ropy's registered shortcut.

Stop the recorded app/editor/input/compositor processes after collecting evidence,
then stop the VM. Retain its disk for subsequent regression runs.

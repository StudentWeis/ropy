# Native Wayland support

Ropy uses a native Wayland window when `WAYLAND_DISPLAY` is set. XWayland and
`DISPLAY` are not required. Native X11 sessions continue to use the X11 backend.
Clipboard capture and immediate paste have separate compositor requirements;
see the capability limits below.

## Version and quick start

This guide describes the implementation merged in [PR #234](https://github.com/StudentWeis/ropy/pull/234),
which addresses [Issue #142](https://github.com/StudentWeis/ropy/issues/142).
Use a build containing that change; older releases may still use XWayland.
No separate Wayland build or feature flag is needed for the standard build.

1. Start Ropy inside your Wayland session with `ropy --show`.
2. Configure activation using the portal, Hyprland integration or a manual
   `ropy --show` binding described below. A tray icon is not required for this path.
3. On tiling desktops, configure a floating rule if you want a popup-sized window.
4. Copy text in another native Wayland application, search for it in Ropy and
   confirm it. Verify clipboard capture and paste separately from window activation.

| Capability | Requirement / validation boundary |
| --- | --- |
| Native window and repeated activation | Tested in Sway with XWayland disabled and `DISPLAY` unset |
| Global shortcut | Supported Hyprland integration, GlobalShortcuts portal, or manual compositor binding |
| Background clipboard capture | Compositor must expose the supported data-control protocol |
| Immediate paste | Compositor must expose the supported virtual-keyboard protocol |
| GNOME, KDE and Niri | Not individually validated; a working window does not establish clipboard or paste support |

## Popup lifetime

Ropy starts hidden and keeps its history, clipboard services, settings and updater
alive between activations. Dismissing a Wayland popup destroys its native window;
activation creates a new window over the retained application state. Window-bound
search and settings controls are recreated. This avoids depending on GPUI's
unsupported Wayland hide/remap operation.

On Linux, `ropy --show` opens the existing instance, or starts one and shows it if
none is running. A subsequent plain `ropy` invocation also opens the existing
instance. Instances are scoped to the display session, and a stale activation
socket is recovered after a crash. X11 sessions without `XDG_RUNTIME_DIR` use a
private temporary directory scoped to the current user and display.

A launcher-provided `XDG_ACTIVATION_TOKEN` is forwarded to the existing instance
and applied through `xdg_activation_v1`. Portal activation tokens use the same
path. The compositor decides whether to grant focus; an untrusted, expired or
missing token does not bypass its focus policy.

## Global shortcuts

### Hyprland 0.55+ with Lua configuration

Ropy automatically registers the shortcut from its settings with Hyprland at
runtime. It receives activation through Hyprland's event socket and requests
focus for its own process after the popup maps. No configuration-file edits or
extra activation process are needed. Changing or clearing the shortcut updates
the runtime binding; configuration reloads restore it while Ropy is running.

The adapter refuses conflicting bindings instead of replacing them. Save failures
retain the previous shortcut. Letters, digits, F1–F24, navigation keys and common
punctuation are supported; unsupported keys fail registration. Physical-keycode
bindings with the same modifiers are conservatively treated as conflicts. Remove
previous manual Ropy bindings before assigning the same chord in Ropy.

On shutdown, Ropy disables only its own Lua binding handle. Disabled handles are
reused on later registration and removed by Hyprland's next configuration reload.
A crash can leave the binding until restart or compositor reload.

### GlobalShortcuts portal

Other Wayland sessions use the desktop's
[GlobalShortcuts portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html).
The desktop may ask the user to approve or choose a shortcut. Ropy's setting is
the **preferred** chord; the portal can assign a different trigger, which is
managed by the desktop. Ropy currently displays the preferred chord in its settings.

A replacement registration must succeed before the saved preference and old
registration are replaced. Rejection or a save failure preserves the previous
registration. Clearing the preference closes the session. Exiting cancels an
unfinished request and closes the active session. Portal events are filtered by
session and shortcut ID, and their activation token is forwarded to the compositor.
Registration errors are recorded in Ropy's log.

### Manual compositor binding

If the desktop does not provide GlobalShortcuts, bind `ropy --show` in its shortcut
settings. Use an absolute executable path if Ropy is not on the compositor's `PATH`.
For Sway:

```ini
bindsym Ctrl+Shift+d exec ropy --show
```

For older Hyprland configurations:

```ini
bind = CTRL SHIFT, D, exec, ropy --show
```

These are manual bindings; changing Ropy's setting does not update them. Ropy does
not attempt X11 global-key grabs in a Wayland session.

## Window placement and scaling

The native window uses `xdg_toplevel` with application ID `com.ropy.app`. Wayland
leaves placement, floating and stacking to the compositor. A tiling compositor
may tile the window unless configured otherwise. For a floating Sway popup:

```ini
for_window [app_id="^com\\.ropy\\.app$"] floating enable
```

The initial requested size is 400 × 550 logical pixels. GPUI's Wayland backend
handles output scaling; Ropy no longer applies the former Hyprland/XWayland
scale override or restarts itself to set `GPUI_X11_SCALE_FACTOR`.

Pinning prevents automatic dismissal. It does not force a native Wayland window
above other applications; configure compositor stacking rules if desired.
Sway's default `focus_on_window_activation urgent` marks an already-visible pinned
window urgent rather than focusing it, even with a valid launcher token. If you
want valid activation requests on the visible workspace to receive focus, Sway
provides `focus_on_window_activation smart`. This is a desktop-wide preference;
Ropy does not change it. A newly mapped popup and a pinned existing window must
be tested separately.

## Capabilities and limits

- Clipboard capture requires the data-control protocol used by
  `clipboard-rs` / `wl-clipboard-rs`. The watcher polls for changes. A working
  window or shortcut portal does not establish background clipboard support.
- Immediate paste uses Enigo's Wayland virtual-keyboard support. The compositor
  must expose that protocol. Only one input backend is connected to avoid
  duplicate pastes. If initialization fails, clipboard write-back remains
  available for manual paste and the failure is logged.
- Activation tokens require `xdg_activation_v1`; without it the window can still
  open, but focus follows the compositor's normal policy.
- GNOME, KDE, Niri and other compositor combinations need separate validation of
  shortcuts, clipboard capture, paste and focus. The Sway test does not certify
  these desktops. A portal implementation alone does not supply data-control or
  virtual-keyboard support.
- macOS and Windows retain their resident native window behavior.

## Validation

See [WAYLAND_VM.md](WAYLAND_VM.md) for the isolated native Sway scenario and the
D-Bus portal fixture. The fixture verifies Ropy's protocol contract; it is not a
GNOME or KDE portal integration test.

For manual verification, focus a plain-text editor, copy a unique marker, activate
Ropy, search for the marker and confirm it. The popup must disappear and return
focus to the editor. With immediate paste enabled, the editor must receive the
marker exactly once. Repeat after the popup has been closed and after copying a
new marker while Ropy is hidden.

Pure tests cover popup state retention and fresh input subscriptions, token
forwarding and validation, shortcut translation and transactional rollback. An
opt-in live test requires a working Hyprland Lua session with virtual-keyboard
support and a free Ctrl+Shift+F12 chord:

```sh
cargo test --bin ropy test_live_hyprland_activation_reload_and_cleanup -- --ignored
```

It injects the shortcut before and after a configuration reload and verifies
cleanup and reuse. Run it only in an isolated test session.

# Wayland session compatibility

Ropy supports an initial hybrid mode for Wayland sessions: the history window
uses XWayland, while clipboard capture/write-back and simulated paste use
Wayland. This is not a native Wayland window implementation. XWayland and a
valid `DISPLAY` are still required, along with `XDG_RUNTIME_DIR`.

## Open history from the compositor

On Linux, `ropy` starts in the tray. `ropy --show` opens the existing instance,
or starts one and shows it if none is running. A subsequent plain `ropy`
invocation also opens the existing instance. Instances are scoped to the display
session, and a crashed process's activation socket is recovered on restart.
For X11 sessions without `XDG_RUNTIME_DIR` (or with an empty value), activation
uses a private directory under the system temporary directory, scoped to the
current user and display. Existing runtime-directory endpoints are unchanged.

Bind `ropy --show` in your compositor's shortcut settings. Use an absolute
executable path if `ropy` is not on the compositor's `PATH`. For a source build,
use the absolute path to `target/debug/ropy`.

Hyprland 0.55+ (Lua configuration):

```lua
hl.bind("CTRL + SHIFT + D", hl.dsp.exec_cmd("ropy --show"))
```

Older Hyprland configurations:

```ini
bind = CTRL SHIFT, D, exec, ropy --show
```

Sway's equivalent command is:

```ini
bindsym Ctrl+Shift+d exec ropy --show
```

These are configuration examples, not changes made by Ropy. The shortcut in
Ropy's settings still uses the X11 global-hotkey backend; it cannot reliably
activate from a native Wayland application. Configure the compositor shortcut
for that workflow. Changing the shortcut in Ropy does not change the compositor
binding.

## Window scaling

On Hyprland, Ropy reads the focused monitor's scale at startup. When
`xwayland:force_zero_scaling` is enabled, Ropy renders at that scale; otherwise
it leaves scaling to the compositor. This avoids GPUI's X11 physical-DPI estimate
making the popup too large. An explicit `GPUI_X11_SCALE_FACTOR` takes precedence.
No desktop configuration is changed.

Restart Ropy after changing display scaling. Moving between monitors with
different scales is not handled dynamically. If Hyprland's IPC is unavailable,
or on other compositors, GPUI's existing X11 scale detection remains in use.

## Capabilities and limits

- Clipboard support requires the data-control protocol used by
  `clipboard-rs` / `wl-clipboard-rs`. Enabling the feature does not guarantee
  background capture on every compositor. The Wayland watcher polls for changes.
- Immediate paste uses Enigo's Wayland virtual-keyboard support. Compositors must
  expose the required protocol. Only one input backend is connected so XWayland
  applications do not receive a duplicate paste. If the Wayland input backend
  cannot initialize, the clipboard remains available for manual paste; the
  failure is recorded in the application log.
- Popup hiding, remapping, and always-on-top requests use X11 window operations.
  In Wayland sessions, the popup is registered as a managed utility window so
  Hyprland can route pointer input to it while keeping it floating. GPUI's
  default override-redirect notification window displayed correctly but did
  not receive mouse input in the tested Hyprland session.
  The compositor may decline activation or always-on-top requests. The current
  GPUI Wayland backend lacks the hide/remap operation needed by a resident popup.
- GNOME, KDE, Sway, and other compositor combinations are not certified by the
  Hyprland test. In particular, shortcut, clipboard, input, and focus capabilities
  need separate validation. Sessions without XWayland are unsupported.
- Native Windows and macOS paths are unchanged.

## Validation

The development build was exercised on Hyprland 0.56.2, Linux x86-64, using an
isolated Ropy configuration and a GTK 3 editor confirmed to use
`GdkWaylandDisplay`. The scenario checks hidden startup, native clipboard
capture, `--show`, Escape dismissal, reopening, clipboard write-back, and
immediate paste of a unique marker exactly once into the native editor.
At 160% display scaling, the default popup measures 400 × 550 logical pixels
(640 × 880 physical pixels).

Unit tests cover single-instance forwarding, stale socket recovery, independent
session endpoints, mutually exclusive paste backends, and compositor scaling
policy with malformed-response fallbacks. The dependency check
protects the Linux Wayland feature flags.

For manual verification, start with a plain-text editor focused, copy a unique
marker, activate Ropy through the compositor binding, and confirm the selected
entry. With immediate paste enabled, the editor must receive the marker once.
Keep focus on the intended destination while the popup is being dismissed.

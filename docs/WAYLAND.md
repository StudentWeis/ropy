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

### Hyprland 0.55+ with Lua configuration

Ropy automatically registers the shortcut from its settings with Hyprland at
runtime. It receives activation through Hyprland's event socket, so the shortcut
works while native Wayland applications are focused. After the popup maps, Ropy
requests focus from Hyprland for its own process. No configuration-file edits
or extra activation process are needed. Changing or clearing the shortcut updates
the runtime binding; configuration reloads restore it while Ropy is running.

The adapter refuses conflicting bindings instead of replacing them. Save failures
retain the previous shortcut and use the existing settings error notification.
Letters, digits, F1–F24, navigation keys and common punctuation are supported;
unsupported keys fail registration. Physical-keycode bindings with the same
modifiers are conservatively treated as conflicts. Remove any previously added
manual Ropy binding before assigning the same chord in Ropy.

On shutdown, Ropy disables only its own Lua binding handle. Disabled handles are
reused on later registration and removed by Hyprland's next configuration reload.
This avoids Hyprland's chord-wide unbind operation removing another application's
binding. A crash can leave the binding until restart or compositor reload.
Registration and reload errors are recorded in Ropy's log. Other Wayland
compositors and older Hyprland configurations still require a manual binding;
this adapter does not implement the GlobalShortcuts portal.

### Other compositor configurations

Bind `ropy --show` in the compositor's shortcut settings. Use an absolute executable
path if Ropy is not on its `PATH`. For older Hyprland configurations:

```ini
bind = CTRL SHIFT, D, exec, ropy --show
```

For Sway:

```ini
bindsym Ctrl+Shift+d exec ropy --show
```

These examples are manual configuration. Changing the shortcut in Ropy does not
update them. On other Wayland desktops, the X11 global-hotkey backend cannot
reliably activate from a native Wayland application.

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

## Hyprland shortcut regression

Pure tests cover key translation, conflict rejection, activation filtering and
transaction rollback. An opt-in live test requires a Lua Hyprland session with
Wayland virtual-keyboard support; it temporarily uses Ctrl+Shift+F12, injects the shortcut through the
Wayland virtual keyboard before and after a configuration reload, and verifies
cleanup and reuse of the disabled handle:

```sh
cargo test --bin ropy test_live_hyprland_activation_reload_and_cleanup -- --ignored
```

Run it only in a test session where that chord is free. It fails rather than
replacing an existing binding. This is compositor integration evidence, separate
from the full clipboard and popup GUI scenario.

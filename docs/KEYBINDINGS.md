# History shortcuts

Edit `config.toml` and restart Ropy to customize history-list shortcuts. Settings
and the global activation hotkey continue to use the existing settings panel.

| Platform | Configuration file |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/ropy/config.toml`, or `~/.config/ropy/config.toml` |
| macOS | `~/Library/Application Support/ropy/config.toml` |
| Windows | `%APPDATA%\ropy\config.toml` |

For example, keep Delete but remove the single-letter D shortcut:

```toml
[keybindings.history]
delete_record = ["delete"]
```

Each entry **replaces all history bindings** for that action. Omitted actions keep
their defaults. An empty array disables that action's history shortcuts; removing
the entry restores the defaults. Multiple keys provide aliases:

```toml
[keybindings.history]
delete_record = ["delete", "ctrl-d"]
toggle_favorite = []
select_next = ["down", "n"]
```

Saving unrelated settings preserves these overrides. Ropy does not write the
resolved default bindings into the configuration. This version loads shortcuts
at startup; it does not provide a shortcut editor or live reloading.

## Actions and defaults

Action IDs are stable configuration names and are not translated.

| Action ID | Default keys | Behavior |
| --- | --- | --- |
| `hide` | `q` | Hide the window |
| `focus_search` | `/` | Focus search |
| `preview_selection` | `space` | Preview the selected record while held, when hold-to-preview is enabled |
| `toggle_window_pin` | `p` | Toggle window pinning where supported |
| `toggle_favorite` | `f` | Toggle the selected record's favorite status |
| `select_left` | `left`, `h` | Previous column in grid mode |
| `select_right` | `right`, `l` | Next column in grid mode |
| `select_prev` | `up`, `k` | Previous record |
| `select_next` | `down`, `j` | Next record |
| `delete_record` | `delete`, `d` | Delete the selected record, with existing confirmation rules |
| `confirm_selection` | `enter` | Confirm the selection using the configured confirmation mode |
| `confirm_selection_plain_text` | `shift-enter` | Confirm as plain text |
| `cycle_filter_next` | `alt-right` | Next content-type filter |
| `cycle_filter_prev` | `alt-left` | Previous content-type filter |
| `toggle_favorites_filter` | `alt-f` | Toggle the favorites filter |
| `confirm_record_1` … `confirm_record_5` | `1` … `5`, respectively | Confirm the corresponding visible record |

## Key syntax and scope

Use one key or combination per string, such as `d`, `delete`, `ctrl-d`, or
`shift-enter`. Separate modifiers with hyphens. Supported modifiers are `ctrl`,
`alt`, `shift`, `cmd`, and `secondary` (`cmd` on macOS, `ctrl` elsewhere).
Lowercase letters are unshifted; `D` means `shift-d`. A sequence such as
`ctrl-k ctrl-d` is not supported.

Keys may be a printable character, `space`, `enter`, `backspace`, `delete`,
`insert`, `home`, `end`, `pageup`, `pagedown`, `up`, `down`, `left`, `right`, or
`f1` through `f24`. Escape, Tab combinations, and the platform quit shortcut
(`cmd-q` on macOS, `alt-f4` elsewhere) are reserved. Duplicate aliases for one
action are deduplicated; assigning the same normalized combination to different
actions is an error. To reuse an existing key, first disable or rebind its current
action.

Overrides apply while the history list has focus. They are suspended in other
panels, confirmation dialogs, text inputs, and native selection controls.
Search retains Up/Down result navigation, Enter and Shift+Enter confirmation,
Alt+Left/Right filter cycling, and Alt+F favorites filtering. Other input keys
keep their editing behavior. Escape retains its existing focus/window behavior.
The help panel reads the effective bindings; keys that apply only in search are
marked “search”. Disabling an action's list shortcuts does not disable its mouse
controls or fixed search shortcuts.

The OS activation shortcut is separate (`hotkey.activation_key` or Settings).
Desktop-reserved or globally registered combinations may be intercepted before
Ropy receives them.

## Configuration errors

An unknown action, unsupported key, reserved key, or conflicting assignment
pauses **all configurable history shortcuts** and displays a persistent error
with the offending action/key. Ropy leaves the overrides intact and does not
silently restore a shortcut you intended to disable. Search controls, Escape,
mouse controls, and the platform quit shortcut remain available. Correct the
configuration and restart Ropy to apply it.

A malformed TOML file, unknown section under `keybindings`, or a value with the
wrong type enters the existing settings recovery mode, which also pauses
history shortcuts and blocks saving over the unreadable configuration.

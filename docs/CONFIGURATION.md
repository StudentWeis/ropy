# Configuration and recovery

Use Settings for normal changes. Ropy saves a candidate configuration before applying supported live changes; a failed save leaves the previous settings active. For history shortcuts and custom themes, see their dedicated guides rather than copying full configuration tables.

## Configuration file

| Platform | Configuration file |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/ropy/config.toml`, or `~/.config/ropy/config.toml` |
| macOS | `~/Library/Application Support/ropy/config.toml` |
| Windows | `%APPDATA%\ropy\config.toml` |

Before editing the file outside the app, quit Ropy and keep a backup. Restart after editing; there is no configuration file watcher. A missing file uses defaults. Omitted fields receive defaults; settings are defined in [the core settings module](../crates/ropy-core/src/config/settings.rs), not by an exhaustive table in this guide.

## Common changes

- [History shortcuts](KEYBINDINGS.md) explains action IDs, replacement versus disabled bindings, scope and conflict recovery. The global activation shortcut is separate.
- [Custom themes](THEMES.md) explains personal theme files, appearance slots and reload/fallback behavior.
- Set confirmation mode in Settings. `copy_to_clipboard` writes the chosen record for manual paste; `paste_immediately` waits for clipboard-write completion, dismisses the popup and sends a paste to the previous application. A failed or timed-out write keeps the popup open with an error.
- History display and retained-storage limits are separate. Search covers the loaded history, not the whole database. Automatic cleanup preserves pinned and favorited records.
- Update preferences control automatic checks and prerelease selection. Package-manager installations can check releases but do not self-replace; see [update behavior and recovery](RELEASING.md#update-behavior-and-recovery).
- On Wayland, activation, clipboard capture and immediate paste have independent protocol requirements. Changing settings cannot add missing compositor support; see [capability limits](WAYLAND.md#capabilities-and-limits).

## Recovery after a load error

If the configuration cannot be read or parsed, Ropy uses protected recovery defaults and shows a persistent alert. Saving, configurable history shortcuts and automatic history cleanup are paused. Recovery display limits do not replace the unreadable retention policy, and the app does not overwrite the broken file with defaults.

1. Keep a copy of the original file and note the reported error. Quit Ropy before repairs.
2. Repair the TOML, value types or file permissions at the path above. For shortcut-specific errors, follow [Configuration errors](KEYBINDINGS.md#configuration-errors); a valid TOML file with an invalid shortcut catalog has a separate recovery path.
3. Restart Ropy. Confirm that the recovery alert is gone, intended settings loaded and shortcuts work before making further changes.

If you intentionally start over, move the backed-up configuration out of the way while Ropy is stopped, then restart with defaults. Review retention settings before resuming normal use: defaults permit automatic cleanup. Do not delete clipboard databases or payload directories to repair a settings file.

Settings serialization, validation and atomic persistence are owned by [core](../crates/ropy-core/src/config/settings.rs); native hotkey validation and live application are desktop responsibilities. See [Architecture](ARCHITECTURE.md#settings-and-presentation) for those boundaries.

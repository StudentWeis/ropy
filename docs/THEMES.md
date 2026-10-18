# Custom themes

In Settings, choose **More → Export theme…** to save a complete copy of the current
theme. Save it in the theme folder, edit its colors, then click **Reload** and
select it. **Open theme folder…** creates and opens that folder when needed.

| Platform | Theme folder |
| --- | --- |
| Linux | `$XDG_CONFIG_HOME/ropy/themes/`, or `~/.config/ropy/themes/` |
| macOS | `~/Library/Application Support/ropy/themes/` |
| Windows | `%APPDATA%\ropy\themes\` |

Each file defines one theme. Only top-level `.toml` files are read. Use lowercase
ASCII letters, digits and hyphens in the filename, for example `midnight.toml`.
Its configuration ID is `user:midnight`; built-in themes retain their existing
IDs and cannot be overwritten by a user file. Changing `theme_name` changes only
the label; renaming the file changes its ID and requires selecting it again.

Use the complete [dark](../assets/themes/ropy-dark.toml) or
[light](../assets/themes/ropy-light.toml) template. All existing fields are required:
`theme_name` must be nonempty, `mode` must be `light` or `dark`, and colors must be
six hexadecimal digits (with an optional `#`). Unknown fields are rejected so
spelling mistakes are visible. Theme files control colors; opacity remains a
separate window setting.

You can also select themes in `config.toml` (restart after editing this file):

```toml
theme = "user:midnight"

[appearance]
follow_system = true
light_theme = "ropy-light"
dark_theme = "user:midnight"
```

The light and dark selectors only show matching themes. When following the
system, changing the inactive slot takes effect on the next system appearance
change. Selecting a theme saves the choice before applying it.

Theme files are read at startup and when you click **Reload**. Reloading updates
the list, names and active colors without restarting. Invalid files are reported
in Settings and do not prevent other themes from loading. A previously loaded
theme remains available for the rest of the process if its file becomes invalid
or disappears; export uses that last valid version too.

On a fresh start, a missing or invalid selected theme falls back to a built-in
theme. The light/dark slots fall back to their corresponding appearance; a fixed
missing theme falls back to the system appearance. The saved selection is never
rewritten by fallback. Repair the file and reload to restore it. A complete
directory read failure retains the previous snapshot.

Exporting does not select or reload the exported file. There is no automatic file
watching or visual color editor in this version.

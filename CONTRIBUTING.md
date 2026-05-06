# Contributing to Ropy

English | [简体中文](./docs/CONTRIBUTING/CONTRIBUTING_ZH.md)

Thank you for considering contributing to Ropy! This document will guide you through adding new themes and language support to the project.

## Release

Releases are managed by `cargo-release`. Run `scripts/update_version.sh` to trigger the flow:

```bash
scripts/update_version.sh patch                   # dry-run a patch bump
scripts/update_version.sh 0.6.0 --execute         # release 0.6.0
scripts/update_version.sh 0.6.0-beta --execute    # pre-release
```

The release pipeline (`cargo release`) automatically:
1. Bumps the version in `Cargo.toml` `[package]`.
2. Syncs the version to `[package.metadata.bundle.bin.ropy]` via `pre-release-replacements`.
3. Runs `scripts/release_prepare.sh` (pre-release hook) which executes `precheck.sh`, `record_build_size.sh`, generates the changelog with `git-cliff`, and verifies `dist plan`.
4. Commits, tags, and pushes — triggering the GitHub Actions release workflow.

## Adding a New Theme

Ropy uses TOML configuration files to define themes. Theme files are located in the `assets/themes/` directory.

### Steps

1. **Create a Theme File**

   Create a new `.toml` file in the `assets/themes/` directory. The filename (without extension) becomes the theme ID. Use lowercase letters and hyphens (e.g., `my-theme.toml`).

2. **Define Theme Content**

   The theme file must include the following fields:

   ```toml
   # Theme display name
   theme_name = "My Theme"

   # Theme mode: light or dark
   mode = "dark"

   # Color definitions (hexadecimal color values)
   background = "#2d2d2d"           # Main background color
   foreground = "#ffffff"           # Main foreground color (text color)
   secondary = "#3d3d3d"            # Secondary background color
   secondary_foreground = "#ffffff" # Secondary foreground color
   border = "#4d4d4d"               # Border color
   accent = "#4d4d4d"               # Accent background color
   accent_foreground = "#ffffff"    # Accent foreground color
   muted = "#3d3d3d"                # Muted background color
   muted_foreground = "#888888"     # Muted foreground color
   input = "#555555"                # Input field background color
   primary = "#6b8cff"              # Primary color
   primary_foreground = "#ffffff"   # Primary foreground color
   primary_hover = "#5a7ae6"        # Primary color hover state
   primary_active = "#4a68cc"       # Primary color active state
   danger = "#f56565"               # Danger/error color
   danger_foreground = "#ffffff"    # Danger foreground color
   popover = "#353535"              # Popover background color
   popover_foreground = "#ffffff"   # Popover foreground color
   selection = "#46506a"            # Selection color
   ring = "#6b8cff"                 # Focus ring color
   list_hover = "#3d3d3d"           # List item hover background color
   list_active = "#4d4d4d"          # List item active background color
   scrollbar_thumb = "#555555"      # Scrollbar thumb color
   ```

3. **Rebuild the Project**

   Theme files are embedded into the binary at compile time, so rebuild the project:

   ```bash
   cargo build --release
   ```

### Examples

Refer to existing theme files:
- `assets/themes/ropy-dark.toml` - Dark theme example
- `assets/themes/ropy-light.toml` - Light theme example
- `assets/themes/nord-light.toml` - Nord color scheme example
- `assets/themes/everforest-night.toml` - Everforest color scheme example

## Adding a New Language

Ropy supports internationalization (i18n). Language files also use TOML format and are located in the `assets/locales/` directory.

### Steps

1. **Create a Language File**

   Create a new `.toml` file in the `assets/locales/` directory. The filename should use the language code (e.g., `fr.toml` for French, `ko.toml` for Korean).

   Language codes should follow the [BCP 47](https://tools.ietf.org/html/bcp47) standard:
   - For regional variants, use `{language-code}-{region-code}` format, e.g.:
     - `zh-CN` - Simplified Chinese
     - `zh-TW` - Traditional Chinese (Taiwan)
     - `pt-BR` - Brazilian Portuguese

2. **Define Language Content**

   The first line of the language file should contain the `language_name` field for display in settings:

   ```toml
   # Language display name (written in the language itself)
   language_name = "Français"

   # Translation key-value pairs
   # Tray menu
   tray_show = "Afficher"
   tray_quit = "Quitter"

   # Main window
   clear_all = "Tout effacer"
   clear_confirm_title = "Effacer tous les enregistrements"
   clear_confirm_message = "Cela supprimera définitivement tous les enregistrements du presse-papiers. Cette action est irréversible."
   clear_confirm_cancel = "Annuler"
   clear_confirm_button = "Effacer"

   # ... other translation keys
   ```

3. **Complete Translation Key List**

   Refer to `assets/locales/en.toml` for all keys that need translation. Here are the main categories:

   - **Tray Menu** (`tray_show`, `tray_quit`)
   - **Main Window** (`clear_all`, `pin`, `unpin`, `filter_*`, `search_*`)
   - **Settings** (`settings_*`)
   - **About** (`about_*`)
   - **Updates** (`update_*`)
   - **Keyboard Shortcuts** (`help_*`)

4. **Rebuild the Project**

   Language files are also embedded at compile time, so rebuild:

   ```bash
   cargo build --release
   ```

### Translation Tips

1. **Keep it concise**: UI space is limited, use compact expressions
2. **Maintain consistency**: Use the same translation for the same concept
3. **Respect user habits**: Use expressions natural to target language speakers
4. **Test the display**: Launch the application to verify translations in the actual interface

### Examples

Refer to existing language files:
- `assets/locales/en.toml` - English (base language)
- `assets/locales/zh-CN.toml` - Simplified Chinese
- `assets/locales/ja.toml` - Japanese

## Code Style

- Follow [Clean Code](https://www.oreilly.com/library/view/clean-code-a/9780136083238/) principles
- Keep code simple (KISS) and avoid repetition (DRY)
- Use `thiserror` to define error types
- Before committing code, run the check script:

   ```bash
   ./scripts/precheck.sh
   ```

## Testing

See the [Testing Documentation](./docs/TESTING.md) for guidelines on writing and running tests.

## Submitting Changes

Ropy follows an **Issue → Branch → PR → Squash Merge** workflow, executed via the local `gh` CLI. The complete, copy-paste-ready SOP lives in the [`ropy-contribution-flow`](./.claude/skills/ropy-contribution-flow/SKILL.md) skill — it is the single source of truth for branch naming, commit conventions, the `scripts/precheck.sh` gate, and the PR self-check. Both human and AI contributors should follow it.

## Reporting Issues

If you find a bug or have a feature suggestion, please submit it on the [Issues](https://github.com/StudentWeis/ropy/issues) page using the matching template.

Thank you for your contribution!

---

**Other Languages**: [简体中文](./docs/CONTRIBUTING/CONTRIBUTING_ZH.md)

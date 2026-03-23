# Multi-Language Support

Ropy provides built‑in support for multiple UI languages, and users can switch between them directly from the settings panel. The system is designed to be simple to extend and efficient at runtime.

## Supported Languages

Ropy discovers available languages at compile time by scanning the `assets/locales` directory. Each `.toml` file represents one locale, and the list shown in the settings panel is generated from those filenames (the human‑readable names are read from a `language_name` key inside the file).

The stock installation ships with at least three files:

- **English** (`en`)
- **简体中文** (Simplified Chinese, `zh-CN`)
- **日本語** (Japanese, `ja`)

Because there is no hard‑coded enum any more, simply adding or removing a `.toml` file is sufficient to change the available languages; no Rust source modifications are required.

## Changing Language

1. Launch the Ropy application.
2. Click the **Settings** button (gear icon) in the top‑right corner of the main window.
3. Scroll down to the **Language** section.
4. Select your preferred language from the drop‑down menu.
5. Click **Save** at the bottom of the settings panel.
6. The UI immediately reloads with the chosen locale.

> ⚠️ Changing the language does **not** require restarting the application; it occurs at runtime.

## Adding a New Language

Adding a new locale is very simple and requires only a translation file:

1. **Create a translation file**
   - Add a new TOML file under `assets/locales/` (for example, `fr.toml` for French).
   - Use `en.toml` or any existing file as a template and translate every key.
   - Include a `language_name = "…"` entry near the top so the UI can show the name.

2. **Rebuild and test**
   - Run `cargo test`. The unit tests automatically iterate over `Language::all()` (which reads the assets) so your new locale will be exercised alongside the defaults.
   - Build and launch the application; the new entry should appear in the Language dropdown immediately. No code edits are necessary since the language list is generated at runtime from the embedded assets.

> 💡 Locale files are bundled with the binary via `rust_embed`, so there is no runtime file I/O. If a requested language file is ever missing the code gracefully falls back to the first available locale.

## Translation File Format

Each translation file is a plain TOML document containing key/value pairs.
Keys correspond to identifiers used throughout the GUI code – for example:

```toml
# Application
app_description = "RustとGPUIで構築されたクリップボードマネージャー"

# Tray menu
tray_show = "表示"
tray_quit = "終了"

# Common toggle labels (used for buttons that switch between two states)
on = "オン"
off = "オフ"
```

> ℹ️ The application name and other static decorations are not stored in the
> translation files; they are defined as constants (`APP_NAME`,
> `ABOUT_BACK_ARROW`, etc.) in the Rust code.

Missing keys are reported at runtime by displaying `[Missing: <key>]` in the UI, and unit tests cover this behaviour.

## Implementation Details

- **Compile‑time embedding:** the `rust_embed` crate scans `assets/locales` and embeds every `.toml` file in the binary. There is no manual `include_str!`; adding or removing files is enough.
- **Language representation:** `Language` is now a thin wrapper around a locale code string. `display_name()` reads the `language_name` key from the corresponding TOML file and returns it, defaulting to the code if absent.
- **Dynamic discovery:** `Language::all()` inspects the embedded asset list, strips the `.toml` suffixes, sorts the codes alphabetically, and returns them. This allows the UI to construct the dropdown without any source‑level maintenance.
- **GPUI Global state:** `I18n` implements the GPUI `Global` trait and is registered at application startup via `cx.set_global(i18n)`. Any component with access to a GPUI context can retrieve translations through the convenience method `I18n::translate(cx, "key")`, eliminating the need to thread an `I18n` reference through component hierarchies. Language changes are applied via `cx.update_global::<I18n, _>(…)`.
- **Runtime behaviour:** lookups are still simple `HashMap<String, String>` accesses and are fast. When changing languages the code attempts to load the requested file and falls back to the first available locale if the file is missing.
- **Persistence:** the selected language is stored in `settings.language` as before; the string value is deserialized back into a `Language` object. The tray menu receives an `&I18n` reference directly because it runs on a platform thread without GPUI context access.
- **Tests:** unit tests cover `Language::display_name`, `Language::all`, translation parsing, missing keys, switching languages, and fallback behaviour. Any new TOML file is automatically exercised by the `Language::all()` tests.

Maintaining translations remains as easy as editing or adding TOML files; the rest of the system adapts without additional work.

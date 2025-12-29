# Multi-Language Support

Ropy now supports multiple languages! Users can easily switch between different languages through the settings panel.

## Supported Languages

- **English** (en)
- **简体中文** (Simplified Chinese, zh-CN)

## How to Use

1. Open Ropy application
2. Click the **Settings** button (gear icon) in the top right corner
3. In the Settings panel, find the **Language** section
4. Click on your preferred language button
5. Click **Save** to apply the changes
6. The UI will immediately switch to the selected language

## Adding New Languages

To add a new language:

1. Create a new TOML file in `assets/locales/` directory (e.g., `fr.toml` for French)
2. Copy the structure from `en.toml` and translate all strings
3. Update `src/i18n/mod.rs`:
   - Add a new variant to the `Language` enum
   - Update the `code()`, `display_name()`, and `all()` methods
   - Add the new language to the `load_language()` match statement
4. Recompile the application

## Translation File Format

Translation files use TOML format with simple key-value pairs:

```toml
# Application
app_name = "Ropy"
app_description = "A clipboard manager built with Rust and GPUI"

# Tray menu
tray_show = "Show"
tray_quit = "Quit"
```

## Implementation Details

- Translations are embedded into the binary at compile time using `include_str!()` for optimal performance
- The i18n system is lightweight and has minimal runtime overhead
- Language selection is persisted in the user's configuration file
- All UI strings are centralized in locale files for easy maintenance

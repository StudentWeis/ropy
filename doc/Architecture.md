**Architecture**

Ropy is a GPUI-based clipboard manager with a small number of focused subsystems. The runtime is centered around `src/app.rs`, which wires together settings, repository access, clipboard I/O, the board UI, hotkeys, and the tray.

## Modules

- **App** (`src/app.rs`): lifecycle orchestration and subsystem startup.
- **Clipboard** (`src/clipboard/`): event-driven clipboard watcher, image persistence, and clipboard writer.
- **Repository** (`src/repository/`): persistent storage, time index, favorites tree, and cleanup logic.
- **GUI** (`src/gui/`): window management, board rendering, search/filter UI, settings/help/about panels, tray, and paste integration.
- **Configuration** (`src/config/`): settings loading, validation, persistence, and OS autostart integration.
- **I18n** (`src/i18n/`): embedded TOML translations with runtime language switching.
- **Updater** (`src/updater/`): release checking, download, install, and restart flow.
- **Utils** (`src/utils/`): logging, hashing, and platform helpers.

## App Orchestration

`app::launch()` is the real entry point after `main.rs` initializes logging.

It performs these steps in order:

1. Creates the GPUI application and initializes `gpui-component`.
2. Loads settings from `config.toml`, validates the configured hotkey, and registers settings as a GPUI global.
3. Loads i18n resources and tray state as globals.
4. Synchronizes OS autostart state with the persisted setting.
5. Opens the repository, loads the most recent `max_history_records`, and stores the repository globally.
6. Starts the clipboard monitor and clipboard writer channels.
7. Creates the main board window and starts the clipboard event handler.
8. Starts the global hotkey listener and tray handler.
9. Triggers an automatic update check when `update.auto_check` is enabled.

The `app` module owns coordination logic so the GUI layer can stay focused on rendering and interaction.

## Clipboard Pipeline

The clipboard pipeline is implemented with async channels and background tasks.

- `listener.rs` uses `clipboard-rs`'s watcher to detect clipboard changes.
- Text entries are forwarded directly as `ClipboardEvent::Text`.
- Image entries are hashed, written to disk, and then forwarded as `ClipboardEvent::Image(path, hash)`.
- `writer.rs` serializes clipboard writes behind a single background task, so confirming an item does not create a new clipboard context each time.
- Consecutive duplicate copies are ignored via `LastCopyState` before they reach storage.

The current end-to-end user flow is implemented for text and images. `ContentType::FilePath` exists in the data model, but it is not fully wired into the board confirm workflow yet.

## Repository Model

The repository uses `sled` with three logical trees:

- `clipboard_records`: postcard-serialized `ClipboardRecord` values keyed by content hash.
- `time_index`: compact chronological index used for fast recent-item selection and cleanup.
- `favorites`: separate favorite membership tree keyed by record ID.

Current record shape:

```rust
pub struct ClipboardRecord {
	pub id: u64,
	pub content: String,
	pub created_at: DateTime<Local>,
	pub content_type: ContentType,
	pub pinned: bool,
}
```

Important implementation details:

- Schema version is currently `3`.
- Schema migration clears records, time index, favorites, and persisted images when the stored schema version changes.
- Text IDs are derived from `content_hash(content, ContentType)`.
- Image IDs use the image content hash, so duplicates update timestamps instead of creating a second record.
- Cleanup runs after new records are saved and removes only old, unpinned, unfavorited records until the repository is back under `max_storage_records`.
- The in-memory board cache only keeps the most recent `max_history_records`, not the full repository.

## GUI and Interaction

The main UI surface is `RopyBoard` in `src/gui/board/`.

It maintains:

- a shared in-memory record cache,
- a filtered index list for the visible view,
- favorite membership loaded from the repository,
- search options (`case_sensitive`, `whole_word`),
- content filters (`All`, `Text`, `Image`, `Favorites`),
- settings state mirrored from persisted configuration,
- update status and notification state.

### Main board features

- Search input with inline case-sensitive and whole-word toggles.
- Filter pills for text, image, and favorites.
- Row actions for favorite, pin, delete, and preview interactions.
- Keyboard navigation via arrow keys or `J`/`K`.
- Quick confirm with `Enter` or `1`-`5`.
- `Space` toggles persistent preview.
- `F` toggles favorite on the selected record.
- `Delete` or `D` removes the selected record.

### Panels

- **Settings**: language, theme, hotkey, display limit, storage limit, autostart, confirm mode, hover preview, auto-check updates, and quick-open buttons for the log/config directories.
- **Help**: keyboard shortcut reference.
- **About**: version info, GitHub link, update status, update download/install action, and restart action after install.

### Confirm modes

`ConfirmMode` changes what happens when a user confirms a record:

- `CopyToClipboard`: write the item back to the clipboard and optionally keep the board pinned.
- `PasteImmediately`: write the item to the clipboard, hide the window, then simulate `Cmd/Ctrl+V` with `enigo`.

Window pinning is intentionally disabled in immediate-paste mode.

## Search Semantics

Search is performed in memory over the loaded board cache.

- Text queries only match `ContentType::Text` records.
- Image filtering ignores the text query and simply returns image items.
- Favorites filtering can be combined with text search, but only favorited text records participate in text matching.
- Result ordering is always pinned-first, then newest-first within each group.

This means search scope is determined by `max_history_records`, while retention scope is determined by `max_storage_records`.

## Configuration and Paths

Settings are serialized as TOML and persisted in `dirs::config_dir()/ropy/config.toml`.

The current settings model includes:

- hotkey activation key,
- display and storage limits,
- theme,
- autostart,
- language,
- update settings,
- hover preview,
- confirm mode.

Repository data is stored under `dirs::data_local_dir()/ropy/`, including `clipboard.db` and persisted image files.

## I18n

Translation files live in `assets/locales/*.toml` and are embedded with `rust-embed`.

- Languages are discovered dynamically from the embedded locale files.
- The `Language` type serializes transparently as the locale code string.
- Language changes are applied at runtime without restarting the app.

## Update Flow

The updater checks GitHub releases in the background.

- Startup auto-check uses `settings.update.auto_check`.
- Manual checks can be triggered from the About panel.
- Downloads run on a dedicated OS thread with progress events sent back to the UI.
- After installation, the app can relaunch itself and then quit the current process.

## Technology Stack

- Clipboard API: [clipboard-rs](https://github.com/ChurchTao/clipboard-rs)
- UI: [gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui)
- UI components: [gpui-component](https://github.com/longbridge/gpui-component)
- Hotkey registration: [global-hotkey](https://github.com/tauri-apps/global-hotkey)
- Tray integration: [tray-icon](https://github.com/tauri-apps/tray-icon)
- Storage: [sled](https://github.com/spacejam/sled)
- Config: [config-rs](https://github.com/rust-cli/config-rs)
- Image handling: [image](https://github.com/image-rs/image)
- Paste simulation: [enigo](https://github.com/enigo-rs/enigo)
- I18n asset embedding: [rust-embed](https://github.com/pyrossh/rust-embed)

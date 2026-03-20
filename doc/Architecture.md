**Architecture**

This project adopts a modular architecture design, mainly divided into the following core modules:

- **App** (`src/app.rs`): Top-level application lifecycle management and subsystem orchestration.
- **Clipboard** (`src/clipboard/`): Responsible for monitoring changes in the system clipboard and writing content back.
- **Repository** (`src/repository/`): Responsible for persisting history records (text, images) using an embedded database.
- **GUI** (`src/gui/`): User interface for searching, previewing, and selecting entries, including window creation and theming.
- **Configuration** (`src/config/`): Manages user settings, preferences, and autostart.
- **I18n** (`src/i18n/`): Internationalization and translation management.
- **Updater** (`src/updater/`): Auto-update checking, downloading, and installation.
- **Utils** (`src/utils/`): Shared utilities such as logging, content hashing, and single-instance enforcement.

# App (Orchestration)

The top-level `app` module (`src/app.rs`) is the central coordinator that:

- Initializes all subsystems (clipboard monitor, repository, GUI, hotkey, tray).
- Loads and validates user settings.
- Synchronizes auto-start state with the OS.
- Wires clipboard events to the repository and GUI refresh pipeline.
- Sets up the global hotkey listener and system tray handler.

This module intentionally lives outside `gui` so that the GUI module can focus solely on rendering and window management.

# Clipboard Management

- Detect changes in clipboard content via an event-driven watcher (`listener.rs`).
- Write text and images back to the clipboard through a dedicated background task (`writer.rs`).
- Deduplication logic: Consecutive identical content (text or image by hash) is not recorded repeatedly.

# Repository (Storage)

- Persist clipboard history records using [sled](https://github.com/spacejam/sled) as the embedded database.
- Support efficient time-based queries via a secondary time index (`time_index.rs`).
- Support configurable storage limits with automatic cleanup of old records.
- Pin records to prevent them from being pruned.

# GUI (Interaction)

- **Window & Theme** (`gui/app.rs`): Embedded assets via `RustEmbed`, window creation, light/dark/system theme switching.
- **Board** (`gui/board/`): Main clipboard history view with search, keyboard navigation, record selection, and deletion.
- **Panels** (`gui/panel/`): Settings, about, and help panels rendered within the main window.
- **Hotkey** (`gui/hotkey.rs`, `gui/hotkey_record.rs`): Global hotkey registration, polling, and interactive hotkey recording in settings.
- **Tray** (`gui/tray.rs`): System tray icon with show/quit menu items.
- **Paste** (`gui/paste.rs`): Simulated paste operation using `enigo` to send Ctrl/Cmd+V.
- **Platform Utils** (`gui/utils.rs`, `gui/x11.rs`): Cross-platform window operations (hide, activate, always-on-top, drag, macOS activation policy).

# Configuration

- Manage user preferences (hotkeys, storage limits, theme, language, auto-update, confirm mode, etc.).
- Provide a settings panel UI for users to modify settings interactively.
- Persist configuration data as TOML in the user's config directory.
- Platform-specific autostart management (login items on macOS, registry on Windows, XDG on Linux).

# I18n (Internationalization)

- TOML-based translation files embedded via `RustEmbed` (`assets/locales/`).
- Support for English, Simplified Chinese, and Japanese.
- Runtime language switching without application restart.

# Updater (Auto-Update)

- Check for new releases from GitHub.
- Download and install updates with progress tracking.
- Configurable auto-check interval and pre-release inclusion.

# Implementation Approach

1. `main.rs` initializes logging and enforces single-instance (Windows), then delegates to `app::launch()`.
2. `app::launch()` initializes all subsystems: settings, repository, clipboard monitor, clipboard writer, GUI window, hotkey listener, and tray handler.
3. The clipboard monitor continuously watches for clipboard changes via an event-driven watcher.
4. When a change is detected, the event handler in `app.rs` applies deduplication logic, persists the record to the repository, updates the in-memory record list, and notifies the GUI to refresh.
5. The global hotkey listener polls for hotkey events and dispatches an `Active` action to show the window.
6. In the GUI, users can search and select historical records using keyboard navigation or mouse.
7. After selecting an entry, the content is written back to the clipboard and optionally an immediate paste is triggered.
8. The settings panel allows users to customize hotkeys, storage limits, theme, language, and other preferences.

# Technology Stack

- System Clipboard API: [clipboard-rs](https://github.com/ChurchTao/clipboard-rs)
- GUI Library: [gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui)
- GUI Components: [gpui-component](https://github.com/longbridge/gpui-component)
- Global Hotkey: [global-hotkey](https://github.com/tauri-apps/global-hotkey)
- Tray Icon: [tray-icon](https://github.com/tauri-apps/tray-icon)
- Database: [sled](https://github.com/spacejam/sled)
- Configuration Management: [config-rs](https://github.com/rust-cli/config-rs)
- Input Simulation: [enigo](https://github.com/enigo-rs/enigo)
- Internationalization: Custom TOML-based i18n with [rust-embed](https://github.com/pyrossh/rust-embed)

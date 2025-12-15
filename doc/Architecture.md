**Architecture**

This project adopts a modular architecture design, mainly divided into the following core modules:

- Clipboard Management: Responsible for monitoring changes in the system clipboard and modifying clipboard content.
- Storage: Responsible for persisting history records (text, images, file paths).
- Interaction: User interface for searching, previewing, and selecting entries.
- Configuration: Manages user settings and preferences.

# Clipboard Management

- Detect changes in clipboard content.
- Write back to the clipboard.

Some extension strategies can be applied:

- Sensitive filtering: Detect if the content comes from password managers (such as 1Password/Bitwarden), and if so, ignore it to protect privacy.
- Deduplication logic: Consecutive identical content should not be recorded repeatedly.

# Storage

- Persist clipboard history records (proprietary data model).
- Support simple queries.
- Support custom sorting rules.

# Interaction

- Global hotkey to invoke the window.
- Tray icon to manage exit and settings.
- List historical records, support search, keyboard navigation, and selection.
- After selection, write back to the clipboard and trigger paste operation.

# Configuration

- Manage user preferences (such as hotkeys, storage limits, etc.).
- Provide a configuration interface for users to modify settings.

# Implementation Approach

1. Initialize the clipboard listener to continuously monitor clipboard content changes.
2. When a change is detected, apply sensitive filtering and deduplication logic.
3. Store the new clipboard content in the database.
4. Provide a global hotkey to invoke the interaction interface.
5. In the interaction interface, users can search and select historical records.
6. After selecting an entry, write back to the clipboard and trigger the paste operation.
7. Provide a configuration interface to allow users to customize settings.

# Technology Stack

- System Clipboard API: [clipboard-rs](https://github.com/ChurchTao/clipboard-rs)
- GUI Library: [gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui)
- GUI Components: [gpui-component](https://github.com/longbridge/gpui-component)
- Global Hotkey: [global-hotkey](https://github.com/tauri-apps/global-hotkey)
- Tray Icon: [tray-icon](https://github.com/tauri-apps/tray-icon)
- Database: [sled](https://github.com/spacejam/sled)
- Configuration Management: [config-rs](https://github.com/rust-cli/config-rs)

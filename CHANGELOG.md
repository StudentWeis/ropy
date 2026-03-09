# Changelog

All notable changes to this project will be documented in this file.

## [0.3.0] - 2026-03-09

### Attention

This update includes a breaking change: the auto-deduplication feature will upgrade the database schema version, which will automatically clear all history records and recreate the database on startup. This change is irreversible, so please make sure to back up your data before upgrading!

破坏性更新提示：自动去重功能将升级数据库模式版本，这将自动清除所有历史记录并在启动时重新创建数据库。此更改不可逆，请确保在升级前备份您的重要数据！

### 🚀 Features
- Implement record pinning functionality and update related UI components by @StudentWeis
- Implement clipboard record deduplication using content hash and update repository logic by @StudentWeis
- Add vim-like key-binding(j/k/d/q) by @StudentWeis
- Add "Open Log File" option in settings and implement log directory retrieval by @StudentWeis

### 🚜 Refactor
- Use a boolean pinned field instead of Category enum for pinning functionality by @StudentWeis
- Split i18n mod.rs into error, language, and translations submodules and re-export public types by @StudentWeis

### ⚙️ Miscellaneous Tasks
- Update image dependency version to 0.25.9 by @StudentWeis
- Update dependencies and toolchain version to 1.94 by @StudentWeis

## [0.2.4] - 2026-02-28

### 🐛 Bug Fixes

- Prevent event propagation on mouse down for settings header buttons in Windows OS by @StudentWeis

### 🚜 Refactor

- Add i18n checkt script and improve localization consistency by @StudentWeis
- Enhance multi-language support and improve language handling by @StudentWeis

### ⚙️ Miscellaneous Tasks

- Update dependencies and improve code organization by @StudentWeis

## [0.2.3] - 2026-02-27

### 🚜 Refactor

- Move about and setting panel to new mod by @StudentWeis
- Beautify the settings page by @StudentWeis

## [0.2.2] - 2026-02-27

### 🚀 Features

- Add auto-update functionality with version checking and installation by @StudentWeis

## [0.2.1] - 2026-02-10

### 🚀 Features

- Integrate tracing for improved logging and error handling by @StudentWeis

### 🐛 Bug Fixes

- Fix windows problem from clippy refactor by @StudentWeis

### 🚜 Refactor

- Clean up code and improve linting compliance across multiple files by @StudentWeis
- Use strict clippy lints to improve the quality of code by @StudentWeis
- Remove debug RSS monitor functionality and related code by @StudentWeis

## [0.2.0] - 2026-01-26

### 🚀 Features

- Add Japanese translations by @ZhBF
- Update cross-platform support CI workflow. by @StudentWeis

### 🐛 Bug Fixes

- Fix missing var name change on linux by @eatradish in [#39](https://github.com/StudentWeis/ropy/pull/39)
- (windows) pin to top functionality by @StudentWeis
- Ensure init gtk and build tray icon on same thread on linux... by @eatradish in [#34](https://github.com/StudentWeis/ropy/pull/34)

### 🚜 Refactor

- Improve the logic of the Settings page and clean the code by @StudentWeis
- Simplify the function start_window_drag import by @StudentWeis
- Improve import organization and simplify code structure across multiple files by @StudentWeis
- Simplify the logic of "pin to top" by @StudentWeis

### ⚙️ Miscellaneous Tasks

- Reorganize imports and update precheck script for nightly compatibility by @StudentWeis
- Update dependencies and improve import organization by @StudentWeis

## [0.2.0-beta] - 2026-01-12

### 🚀 Features

- Add support for preserving scroll position during record deletion by @StudentWeis
- Add linux platform support (#31) by @eatradish in [#31](https://github.com/StudentWeis/ropy/pull/31)

### 🐛 Bug Fixes

- Fix build on non linux platform by @eatradish in [#32](https://github.com/StudentWeis/ropy/pull/32)
- Handle settings exit in hide action by @StudentWeis in [#30](https://github.com/StudentWeis/ropy/pull/30)
- Clean up auto-start manager logic in windows by @StudentWeis

### 🚜 Refactor

- Reorganize tray and app module imports and functions by @StudentWeis

### 📚 Documentation

- Update README and TODO by @StudentWeis

### New Contributors

- @eatradish made his first contribution in [#32](https://github.com/StudentWeis/ropy/pull/32)

## [0.2.0-alpha] - 2026-01-04

### 🚀 Features

- Implement preview toggle for clipboard records with hotkey by @StudentWeis in [#27](https://github.com/StudentWeis/ropy/pull/27)
- Add preview functionality for clipboard records by @StudentWeis
- Add about panel with version info and GitHub link by @StudentWeis
- Add hotkey configuration and validation with user hints by @StudentWeis
- Support i18n by @Copilot
- Support auto startup in silent mode by @ZhBF
- Keep window open after copying when "Pin to Top" is enabled by @StudentWeis
- Add rust-embed for asset management and include clear-all and pin-to-top SVG icons by @StudentWeis
- Implement window pinning functionality and add gpui-component-assets by @StudentWeis
- Enhance tray icon functionality with left-click event handling by @StudentWeis

### 🐛 Bug Fixes

- Use current max history records from settings as fallback in RopyBoard by @StudentWeis
- Failed to initialize tray icon on auto startup by @ZhBF
- Clear all records will clear the last copy state by @StudentWeis
- Ensure proper use of image record click operations and refactor the clipboard mod by @StudentWeis
- Deduplicate clipboard images on Windows by @ZhBF

### 🚜 Refactor

- Improve image tooltip handling by @StudentWeis
- Refresh window after copy something by @StudentWeis
- Enhance load_initial_records to use configurable max history records and streamline tray handler action dispatching by @StudentWeis
- Remove gpui-component-assets to reduce the app size by @StudentWeis
- Update clipboard and tray handling to use async execution model by @StudentWeis

### 📚 Documentation

- Update doc of clipboard listener to notify UI for refresh after record update by @StudentWeis
- Add Chinese version of README by @Copilot
- Add issue templates for bug reports and feature requests by @StudentWeis

## [0.1.4] - 2025-12-22

### 🐛 Bug Fixes

- Figure the problem of test of clear in repository that deletes user data by @StudentWeis

### 🚜 Refactor

- Simplify error message formatting across multiple files by @StudentWeis
- Streamline hotkey listener setup and improve event handling by @StudentWeis
- Integrate async-channel for clipboard image handling and enhance clipboard listener by @StudentWeis
- Enhance clipboard image handling and simplify record delete by @StudentWeis
- Add script to build and package macOS DMG for Ropy by @StudentWeis

### 📚 Documentation

- Update concurrency documentation to clarify threading model and message passing flow by @StudentWeis

## [0.1.3] - 2025-12-20

### 🐛 Bug Fixes

- Fix window dragging on Windows by @ZhBF

### 🚜 Refactor

- Simplify the logic of render record list and clean the code by @StudentWeis

### 📚 Documentation

- Update README for clarity and remove outdated images by @StudentWeis

## [0.1.2] - 2025-12-19

### 🚀 Features

- Enhance clipboard copy with background processing and update TODO list by @StudentWeis
- Add build script for Windows icon by @ZhBF

### 🐛 Bug Fixes

- When hiding the window on Windows would leave a minimized artifact on the desktop by @ZhBF
- Use conditional compilation for Windows target in build script by @ZhBF

## [0.1.1] - 2025-12-18

### 🚀 Features

- Add support for hex color parsing in clipboard content display by @StudentWeis

### 🐛 Bug Fixes

- Correct usage of hide_window function in record click handler by @StudentWeis

### 📚 Documentation

- Add usage instructions to README for clipboard management features by @StudentWeis
- Update README with enhanced visuals and installation instructions; add new asset images by @StudentWeis

## [0.1.0] - 2025-12-18

### 🚀 Features

- Add custom GitHub runners for macOS builds in dist configuration by @StudentWeis
- Add initial configuration for CI/CD with GitHub Actions and dist by @StudentWeis
- Implement auto-start functionality with configuration options by @StudentWeis
- Add theme management with light, dark, and system options by @StudentWeis
- Refactor RopyBoard component and add settings management by @StudentWeis
- Implement system tray functionality and enhance README with new features by @StudentWeis
- Add support for saving and managing image clipboard records by @StudentWeis
- Add image handling to clipboard with saving and retrieval functionality by @StudentWeis
- Add keyboard navigation and confirmation for hiding records in RopyBoard by @StudentWeis
- Add keyboard navigation for selection in GUI by @StudentWeis
- Implement record deletion functionality in clipboard repository and update GUI rendering by @StudentWeis
- Update timestamp handling to use local time in clipboard models and repository by @StudentWeis
- (windows) simplify memory reporting in RSS monitor and improve single instance window activation by @StudentWeis
- Apply dark theme to window creation and remove text color from search input by @StudentWeis
- Enhance clipboard repository with search functionality by @StudentWeis
- Enhance windows single instance handling by activating existing window or showing message box by @StudentWeis
- Add repository handling to RopyBoard; implement clear history functionality by @StudentWeis
- Refactor RopyBoard to remove visibility handling; add active window action by @StudentWeis
- Add single instance handling for Windows; update README for cross-platform support by @StudentWeis
- Improve window management for RopyBoard with OS-specific handling; enhance hotkey registration logic by @StudentWeis
- Add Windows support with new dependencies and hotkey handling; improve window management by @StudentWeis
- Enhance RopyBoard with quit action and update visibility toggle method by @StudentWeis
- Add focus out handling to RopyBoard for automatic window hiding by @StudentWeis
- Enhance RopyBoard with focus handling and window integration; add key binding for visibility toggle by @StudentWeis
- Update .gitignore and enhance Cargo.toml with metadata; modify window creation to remove titlebar by @StudentWeis
- Add macOS support for accessory application mode by @StudentWeis
- Enhance RopyBoard with visibility toggle and update window creation by @StudentWeis
- Add clipboard writer module and list of clib records and refactor the code by @StudentWeis
- Implement clipboard history repository with persistence and deduplication by @StudentWeis
- Add global hotkey functionality and integrate with clipboard monitoring by @StudentWeis
- Implement clipboard listener and integrate with GUI by @StudentWeis
- Add sysinfo dependency and implement RSS monitoring in debug mode by @StudentWeis

### 🐛 Bug Fixes

- Enhance macOS bundle build process and output handling by @StudentWeis
- Resolve unused imports and variable warnings by @ZhBF
- Use dark-light crate for Windows theme detection by @StudentWeis in [#2](https://github.com/StudentWeis/ropy/pull/2)
- Use dark-light crate for Windows theme detection to resolve build errors by @ZhBF
- Improve focus handling in settings button by @StudentWeis
- Avoid the conflict between the search input number hotkey by @StudentWeis
- (macOS) add codesign identity for macOS application packaging by @StudentWeis

### 🚜 Refactor

- Update TODO list and improve active window handling in RopyBoard by @StudentWeis
- Update AppTheme::get_theme to simplify system theme detection logic by @StudentWeis
- Update clipboard listener documentation and improve hotkey monitor initialization by @StudentWeis
- Check duplicate content in clipboard mod instead of repository mod by @StudentWeis
- Improve code formatting and enhance focus handling in RopyBoard by @StudentWeis
- Improve code organization and enhance theme handling in GUI components by @StudentWeis
- Update README and code comments to improve clarity and consistency by @StudentWeis
- Change clipboard listener to use event-driven watching by @StudentWeis
- Divide repository mod by @StudentWeis
- Clean the code by @StudentWeis
- Clean the code by @StudentWeis

### ⚙️ Miscellaneous Tasks

- Bump version to 0.1.0 by @StudentWeis
- Update README and TODO files, enhance image display in GUI by @StudentWeis

### New Contributors

- @StudentWeis made their first contribution
- @ZhBF made their first contribution

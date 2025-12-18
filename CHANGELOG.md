# Changelog

All notable changes to this project will be documented in this file.

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

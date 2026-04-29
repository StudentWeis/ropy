# Changelog

All notable changes to this project will be documented in this file.

## [0.5.0-rc.3] - 2026-04-29

### 🚀 Features
- Make grid layout more compact (#58) by @StudentWeis in [#58](https://github.com/StudentWeis/ropy/pull/58)

### 🚜 Refactor
- Split records list modules (#67) by @StudentWeis in [#67](https://github.com/StudentWeis/ropy/pull/67)
- Apply §5 high-ROI cleanups from CODE_REVIEW.md (#65) by @StudentWeis in [#65](https://github.com/StudentWeis/ropy/pull/65)
- Consolidate assets/icon into assets/icons (#63) by @StudentWeis in [#63](https://github.com/StudentWeis/ropy/pull/63)
- Parallelize test suite, serialize only stateful autostart tests (#61) by @StudentWeis in [#61](https://github.com/StudentWeis/ropy/pull/61)

### 📚 Documentation
- Remove outdated code review content by @StudentWeis
- Remove completed storage backend consolidation section from CODE_REVIEW.md by @StudentWeis

### ⚙️ Miscellaneous Tasks
- Update skill doc with git prune command by @StudentWeis

## [0.5.0-rc.2] - 2026-04-25

### 🐛 Bug Fixes
- Move record click handler to card level (#56) by @StudentWeis in [#56](https://github.com/StudentWeis/ropy/pull/56)

### 💼 Other
- Upgrade tray-icon dependency to v0.22.1 by @StudentWeis

### 🚜 Refactor
- Genericize repository storage (#54) by @StudentWeis in [#54](https://github.com/StudentWeis/ropy/pull/54)
- Replace per-function expect_used allow with module-level cfg_attr by @StudentWeis in [#51](https://github.com/StudentWeis/ropy/pull/51)
- Split start_tray_handler into focused helpers (#48) by @StudentWeis in [#48](https://github.com/StudentWeis/ropy/pull/48)

### 📚 Documentation
- Update README with new features and layout options by @StudentWeis

### ⚡ Performance
- Use opt-level 3 in release profile for better runtime performance (#52) by @StudentWeis in [#52](https://github.com/StudentWeis/ropy/pull/52)

### ⚙️ Miscellaneous Tasks
- Drop pr-title workflow, bump checkout to v6, slim contribution skill by @StudentWeis in [#49](https://github.com/StudentWeis/ropy/pull/49)
- Add contribution workflow and issue templates for Ropy by @StudentWeis
- Update dependencies for improved stability and performance by @StudentWeis

## [0.5.0-rc.1] - 2026-04-18

### 🚀 Features
- Implement grid layout for records with masonry placement and enhance search functionality by @StudentWeis
- Enhance Windows support with additional UI features and improve window geometry handling by @StudentWeis
- Add layout mode selection with list/grid options by @StudentWeis

### 🐛 Bug Fixes
- Add auto-hide behavior for secondary panels and corresponding tests by @StudentWeis

### 💼 Other
- Add release management script and config updates by @StudentWeis

### 🚜 Refactor
- Improve code structure and add tests for repository modules by @StudentWeis
- Remove unused panel auto-hide logic by @StudentWeis
- Update imports and method calls for consistency by @StudentWeis
- Update Rust toolchain to 1.95.0 and refactor hotkey handling with cfg_select by @StudentWeis

### 📚 Documentation
- Add SKILL.md files for gpui layout, style, performance, and styling skills by @StudentWeis

### ⚙️ Miscellaneous Tasks
- Add symbolic links for claude skills and docs by @StudentWeis
- Update dependencies and improve formatting in Cargo.toml and Cargo.lock by @StudentWeis

## [0.5.0-beta] - 2026-04-16

### 🚀 Features
- Add success notification for settings updates by @StudentWeis

### 🐛 Bug Fixes
- Add InteractiveElement import in common.rs by @ZhBF
- Add new SVG icons for circle check, circle x, and triangle alert by @StudentWeis

### 💼 Other
- Remove unused rtk related configurations and scripts by @StudentWeis

### 🚜 Refactor
- Simplify list item styling logic by @StudentWeis
- Simplify social link button rendering logic, remove border by @StudentWeis
- Add cfg target for InteractiveElement import by @StudentWeis
- Add icon allowlist and common panel components by @StudentWeis
- Enhance about panel layout with border styling by @StudentWeis

### 📚 Documentation
- Add scoop extra bucket instruction by @StudentWeis

### 🎨 Styling
- Remove spaces around redirection operators in shell scripts by @StudentWeis

## [0.5.0-alpha] - 2026-04-09

### 🚀 Features
- Add support for RTF/HTML by @StudentWeis

### 🚜 Refactor
- Update default clipboard backend from sled to redb by @StudentWeis

## [0.4.5] - 2026-04-08

### 🚀 Features
- Add prerelease update option and simplify architecture docs by @StudentWeis
- Add social media links in about panel by @StudentWeis

### 🚜 Refactor
- Split large modules into smaller components for better maintainability by @StudentWeis
- Replace booleans with ActivePanel enum for panel visibility management by @StudentWeis
- Simplify record filtering logic and improve sync handling by @StudentWeis
- Remove unused dhat heap profiling code by @StudentWeis
- Update clipboard event handling to use unit type and improve async task management by @StudentWeis
- Simplify click event handling in header and updater UI components by @StudentWeis
- Use weak references and improve async handling in GUI components by @StudentWeis
- Add storage validation and improve logging setup by @StudentWeis
- Adjust padding in settings header layout by @StudentWeis
- Update links in CONTRIBUTING files and adjust settings panel layout by @StudentWeis
- Update color values in nord-light theme configuration by @StudentWeis
- Split board module into smaller components by @StudentWeis

### 📚 Documentation
- Add gpui patterns and testing skills docs by @StudentWeis
- Add comprehensive GPUI element skill documentation by @StudentWeis
- Add SKILL.md files for gpui-action and gpui-event skills by @StudentWeis
- Add comprehensive entity management documentation and project review report by @StudentWeis
- Update image link for Ropy Dark in README by @StudentWeis

## [0.4.4] - 2026-04-08

### 🚀 Features
- Instantly save configuration by @StudentWeis

### 🐛 Bug Fixes
- Handle missing OUT_DIR and invalid icon path in Windows resource compilation by @StudentWeis
- Update search field background color by @StudentWeis

### 💼 Other
- Upgrade tray-icon, semver, zip versions in Cargo.toml by @StudentWeis

### 🚜 Refactor
- Add section labels for settings panel by @StudentWeis
- Move settings editor fields into dedicated struct by @StudentWeis

### 📚 Documentation
- Add windows installation instructions for ropy by @StudentWeis
- Add star history chart to READMEs by @StudentWeis

### ⚙️ Miscellaneous Tasks
- Add open directories setting option by @StudentWeis

## [0.4.3] - 2026-04-07

### 🚀 Features
- Add option to clear only ordinary clipboard records by @StudentWeis
- Add window opacity setting functionality by @StudentWeis

### 🚜 Refactor
- Adjust preview limits for records list display by @StudentWeis
- Remove unused clear button and add clear history setting functionality by @StudentWeis
- Change setting for immediate paste confirmation mode by @StudentWeis

### 📚 Documentation
- Add contributing guidelines in English and Chinese by @StudentWeis
- Move testing docs and update references by @StudentWeis
- Update README images and features description by @StudentWeis

### ⚙️ Miscellaneous Tasks
- Update locale keys for clear history setting across multiple languages by @StudentWeis
- Update dependencies and use static xz2 by @StudentWeis
- Reorder target platforms in dist configuration by @StudentWeis

## [0.4.2] - 2026-04-01

### 🚀 Features

- Add redb backend and improve abstraction layers by @StudentWeis
- Add file path support and related UI updates by @StudentWeis

### 🐛 Bug Fixes

- Improve error handling and logging across multiple modules by @StudentWeis

### 💼 Other

- Add dhat heap profiling support and related docs updates by @StudentWeis
- Update Cargo.toml dependencies and script paths by @StudentWeis

### 🚜 Refactor

- Improve image handling and thumbnail generation logic by @StudentWeis
- Extract hardcoded values into constants and improve code readability by @StudentWeis
- Cache display names for themes and languages by @StudentWeis

### 🧪 Testing

- Add unit tests for clipboard and updater modules by @StudentWeis

### ⚙️ Miscellaneous Tasks

- Add script for checking theme files consistency and validity of color values. by @StudentWeis

## [0.4.1] - 2026-04-01

### 🚀 Features

- Add search icon by @StudentWeis
- Enhance theme system and add more themes by @StudentWeis
- Exclude pinned and favorite records from history limits by @StudentWeis

### 🚜 Refactor

- Replace Mutex with RwLock for shared records and update synchronization utilities by @StudentWeis
- Add lock_or_recover function for safe mutex handling and replace duplicated lock handling with lock_or_recover in multiple modules by @StudentWeis
- Implement CurlCommandBuilder to streamline HTTP requests by @StudentWeis
- Remove dead code and unused error variants across multiple modules by @StudentWeis
- Optimize cleanup process in ClipboardRepository to reduce I/O frequency by @StudentWeis
- Enhance tray event handling with async channels and dedicated forwarders by @StudentWeis
- Replace hotkey polling with blocking channel bridges by @StudentWeis

## [0.4.0] - 2026-03-30

### 🚀 Features
- Add favorite functionality and improve UI elements by @StudentWeis

### 🐛 Bug Fixes
- Fix panic with message: "Failed to get reply" ... by @eatradish

### 🚜 Refactor
- Adjust list state measurement to improve scroll performance by @StudentWeis
- Add global tray state registration and adjust storage limit defaults by @StudentWeis
- Reduce the logo.png file size and enhance tray icon loading functionality by @StudentWeis
- Optimize record filtering and improve rendering logic by @StudentWeis
- Reduce size of image thumbnail by @StudentWeis
- Refuce sled memory cache and add memory profiling script by @StudentWeis
- Improve settings panel rendering logic by @StudentWeis

### 📚 Documentation
- Update README and Architecture docs by @StudentWeis
- Add doc of RTF support by @StudentWeis

## [0.3.6] - 2026-03-24

### 🚀 Features

- Enhance update process with progress reporting and add RTK documentation by @StudentWeis
- Implement GPUI global state for I18n and update translation access by @StudentWeis
- Use global instead of Arc<Rwlock<Setting>> by @StudentWeis

### 🚜 Refactor

- Streamline search button creation and improve button styling by @StudentWeis
- Simplify search functionality by removing match modes and implementing whole word search by @StudentWeis
- Implement GlobalRepository for shared clipboard access across components by @StudentWeis
- Streamline I18n loading and registration in application launch by @StudentWeis
- Replace AsyncApp with App context in clipboard and GUI modules by @StudentWeis
- Organized the components in the board module by @StudentWeis

### ⚙️ Miscellaneous Tasks

- Add more pre-commit checks and rename script dir by @StudentWeis
- Remove ai doc and clean the code by @StudentWeis

## [0.3.5] - 2026-03-20

### 🚀 Features

- Update tray icon handling and improve menu translation logic by @StudentWeis
- Add clear_search method to reset search input and blur focus when hiding the window by @StudentWeis
- Implement advanced search options with case sensitivity and match modes by @StudentWeis

### 🐛 Bug Fixes

- Ensure notify window when unpin by @StudentWeis
- Update search input placeholder to use internationalization by @StudentWeis

### ⚙️ Miscellaneous Tasks

- Simplify some titles in English, Japanese, and Chinese translations by @StudentWeis
- Add pre-commit hooks and scripts for code checks and version updates by @StudentWeis

## [0.3.4] - 2026-03-17

### 🚀 Features

- Add user-friendly network error message for update failures by @StudentWeis
- Enhance list item rendering with improved hover and selection styles by @StudentWeis

### 💼 Other

- Add script to record build sizes and append to CSV by @StudentWeis
- Remove issue templates from repository by @StudentWeis
- Update dependencies and CI configurations for improved stability by @StudentWeis
- Add icon file checker script by @StudentWeis

### 🚜 Refactor

- Add records list module and refactor clipboard rendering logic by @StudentWeis

### 🧪 Testing

- Add comprehensive tests for repository and time index functionality by @StudentWeis

## [0.3.3] - 2026-03-13

### 🚀 Features

- Add content type filter for clipboard history with toggle buttons by @StudentWeis
- Add confirmation dialog for clear all button by @StudentWeis
- Update settings to open log and config directories by @StudentWeis
- Separate max storage and display records settings by @StudentWeis

### 🚜 Refactor

- Simplify clipboard event handling and key binding logic by @StudentWeis
- Extract app orchestration from gui into top-level app module by @StudentWeis
- Move clipboard event handler from listener.rs to gui/app.rs by @StudentWeis
- Reorganize logging and content hash utilities by @StudentWeis
- Remove update section from settings panel to about panel by @StudentWeis

### ⚙️ Miscellaneous Tasks

- Update image links for Ropy Dark and Light in README by @StudentWeis
- Update dependencies for image, tempfile, and enigo packages by @StudentWeis

## [0.3.2] - 2026-03-12

### 🚀 Features

- Add hotkey recording functionality by @StudentWeis
- Add validation and warning for max history input in settings by @StudentWeis
- Enhance settings panel layout with improved scrolling and size handling by @StudentWeis
- Add notifications for settings save status by @StudentWeis
- Add configurable confirm action modes for clipboard interactions by @StudentWeis
- Enhance record filtering by search query in RopyBoard by @StudentWeis

### 🐛 Bug Fixes
- Remove vim key bindings in action to avoid serach error by @StudentWeis

## [0.3.1] - 2026-03-10

### Attention

This update includes a breaking change: the auto-deduplication feature will upgrade the database schema version, which will automatically clear all history records and recreate the database on startup. This change is irreversible, so please make sure to back up your data before upgrading!

破坏性更新提示：自动去重功能将升级数据库模式版本，这将自动清除所有历史记录并在启动时重新创建数据库。此更改不可逆，请确保在升级前备份您的重要数据！

### 🚀 Features

- Add prek configuration by @StudentWeis
- Add hover preview settings and UI toggle by @StudentWeis
- Enhance records list rendering with scrollbar by @StudentWeis
- Add help panel with keyboard shortcuts and corresponding translations by @StudentWeis
- Implement lightweight time index for efficient chronological queries by @StudentWeis
- Migrate serialization from serde_json to postcard for ClipboardRecord

### 🚜 Refactor

- Update async handling in RopyBoard for background tasks by @StudentWeis
- Remove sysinfo dependency by @StudentWeis
- Change filtered_records to use Arc by @StudentWeis

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

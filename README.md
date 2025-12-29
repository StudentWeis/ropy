<p align="center"><img src="assets/logo.png" alt="Ropy Logo" width="20%"></p>

<h2 align="center"><em><strong>R</strong>opy <strong>O</strong>rganizes <strong>P</strong>revious <strong>Y</strong>anks</em></h2>

<p align="center">
<a href="https://github.com/studentweis/ropy/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="License"></a>
<a href="https://github.com/studentweis/ropy/releases"><img src="https://img.shields.io/github/v/release/studentweis/ropy" alt="Release"></a>
<a href="https://rust-lang.org"><img src="https://img.shields.io/badge/language-Rust-orange" alt="Language"></a>
</br>
<a href="https://github.com/studentweis/ropy"><img src="https://img.shields.io/github/stars/studentweis/ropy?style=social" alt="Stars"></a>
<a href="https://github.com/studentweis/ropy/issues"><img src="https://img.shields.io/github/issues/studentweis/ropy" alt="Issues"></a>
</p>

<p align="center">A cross-platform native clipboard manager in pure Rust.</p>

<p align="center">
English | <a href="doc/README_ZH.md">简体中文</a>
</p>

<p align="center">
<img src="https://s2.loli.net/2025/12/19/iqnFu2pdNogyA7P.png" alt="Ropy Dark" width="45%">
<img src="https://s2.loli.net/2025/12/19/OaiXxnfGQLRvH5T.png" alt="Ropy Light" width="45%">
</p>

# Features

- Cross-platform support: Windows and macOS.
- Native GUI application built with Zed's GPUI.
- Easy-to-use, lightweight and fast.
- Search to quickly find previous records.
- Auto start on system boot.
- Keyboard shortcuts for quick access.

# Installation

## Pre-built Binaries

You can download the latest pre-built binaries from the [Releases](https://github.com/StudentWeis/ropy/releases) page.

### macOS

After downloading the `.dmg` file and dragging Ropy.app to the Applications folder, you may need to remove the quarantine attribute to run the application without issues. Open Terminal and run the following commands:

```sh
xattr -rc /Applications/Ropy.app
sudo xattr -r -d com.apple.quarantine /Applications/Ropy.app
```

## Building from Source

Make sure you have Rust installed. You can install Rust using [rustup](https://rustup.rs/).

1. Clone the repository:

```bash
git clone https://github.com/StudentWeis/ropy.git
cd ropy
```

2. Build the project:

```bash
cargo build --release
```

3. Run the application:

```bash
./target/release/ropy
```

# Usage

- Launch the application, and it will start monitoring your clipboard.
- Use the global hotkey(Ctrl/Control + Shift + D) of tray icon to access the clipboard history.
- Click on any record or use 1/2/3/4/5 to select a record using keyboard to copy it back to the clipboard.
- Use the search bar to filter clipboard records.

# Acknowledgements

- Inspired by other clipboard managers like Ditto, Maccy & CopyQ.
- Thanks to the Rust community for their support and libraries.
- System Clipboard API: [clipboard-rs](https://github.com/ChurchTao/clipboard-rs)
- GUI Library: [Zed's gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui)
- GUI Components: [gpui-component](https://github.com/longbridge/gpui-component)
- Global Hotkey: [global-hotkey](https://github.com/tauri-apps/global-hotkey)
- Tray Icon: [tray-icon](https://github.com/tauri-apps/tray-icon)
- Database: [sled](https://github.com/spacejam/sled)
- Configuration Management: [config-rs](https://github.com/rust-cli/config-rs)

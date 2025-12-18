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

<p align="center">A cross-platform native clipboard GUI manager in pure Rust.</p>

<p align="center">
<img src="assets/ropy-dark.png" alt="Ropy Dark" width="45%" style="border:1px solid rgba(0,0,0,0.12); box-shadow:0 8px 24px rgba(0,0,0,0.12); border-radius:8px; padding:4px;">
<img src="assets/ropy-light.png" alt="Ropy Light" width="45%" style="border:1px solid rgba(0,0,0,0.12); box-shadow:0 8px 24px rgba(0,0,0,0.12); border-radius:8px; padding:4px;">
</p>

# Features

- Cross-platform support: Windows and macOS.
- Native GUI application built with Zed's GPUI.
- Lightweight and fast.
- Easy-to-use interface for managing clipboard history.
- Search functionality to quickly find previous clipboard entries.
- Auto start on system boot.
- Keyboard shortcuts for quick access.

# Installation

## Pre-built Binaries

You can download the latest pre-built binaries from the [Releases](https://github.com/StudentWeis/ropy/releases) page.

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
- Click on any entry or use 1/2/3/4/5 to select an entry using keyboard to copy it back to the clipboard.
- Use the search bar to filter clipboard entries.

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

<p align="center"><img src="assets/logo.png" alt="Ropy Logo" width="20%"></p>

<h2 align="center"><em><strong>R</strong>opy <strong>O</strong>rganizes <strong>P</strong>revious <strong>Y</strong>anks</em></h2>

<p align="center">
<a href="https://github.com/studentweis/ropy/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="License"></a>
<a href="https://github.com/studentweis/ropy/releases"><img src="https://img.shields.io/github/v/release/studentweis/ropy" alt="Release"></a>
<a href="https://rust-lang.org"><img src="https://img.shields.io/badge/language-Rust-orange" alt="Language"></a>
<br>
<a href="https://github.com/studentweis/ropy"><img src="https://img.shields.io/github/stars/studentweis/ropy?style=social" alt="Stars"></a>
<a href="https://github.com/studentweis/ropy/issues"><img src="https://img.shields.io/github/issues/studentweis/ropy" alt="Issues"></a>
</p>

<p align="center">Cross-platform, lightweight clipboard manager written in Rust and GPUI.</p>

<p align="center">English | <a href="doc/README/README_ZH.md">简体中文</a></p>

<p align="center">
<img src="https://www.imgur.la/images/2026/03/13/pic_1773332541063.png" alt="Ropy Dark" width="40%">
<img src="https://www.imgur.la/images/2026/03/13/pic_1773332561411.png" alt="Ropy Light" width="40%">
</p>

## Features

- Native GUI application built with Zed's GPUI.
- Cross-platform: Windows, macOS & Linux(X11).
- Easy-to-use, lightweight and fast.
- Search/Pin/Preview/Autostart/Shortcuts/Autoupdate/Record-Pin.

## Installation

### Pre-built Binaries

Download the latest binaries from the [Releases](https://github.com/StudentWeis/ropy/releases) page.

### macOS

After downloading the `.dmg` file and dragging Ropy.app to the Applications folder, you may need to remove the quarantine attribute to run the application without issues. Open Terminal and run the following commands:

```sh
xattr -rc /Applications/Ropy.app
sudo xattr -r -d com.apple.quarantine /Applications/Ropy.app
```

### Build from source

Ensure you have Rust installed (via `rustup`). Then:

```bash
git clone https://github.com/StudentWeis/ropy.git
cd ropy
cargo build --release
./target/release/ropy
```

## Usage

- Launch Ropy to start recording clipboard history.
- Use the global hotkey or the tray icon to open the history window.
- Click a record or press numeric keys (`1`-`5`) or `Enter` to paste.
- Press `Space` to preview an entry without pasting.
- Use the search bar to filter records by content.
- Pin entries to keep them from being pruned when the history limit is reached.

## Limitations (Not planned)

- The system clipboard does not expose the original application source for copied items.
- Plugin/extension system: no current plans — Ropy focuses on simplicity and small footprint.
- Cloud sync: not supported at this time.
- Command-line mode: Ropy is designed primarily as a GUI application.

## Acknowledgements

- Inspired by clipboard managers such as Ditto, Maccy and CopyQ.
- Thanks to the Rust community and all upstream projects used by Ropy.
- System Clipboard API: [clipboard-rs](https://github.com/ChurchTao/clipboard-rs)
- GUI library: [Zed's gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui)
- GUI components: [gpui-component](https://github.com/longbridge/gpui-component)
- Global hotkey: [global-hotkey](https://github.com/tauri-apps/global-hotkey)
- Tray icon helper: [tray-icon](https://github.com/tauri-apps/tray-icon)
- Embedded DB: [sled](https://github.com/spacejam/sled)
- Configuration: [config-rs](https://github.com/rust-cli/config-rs)

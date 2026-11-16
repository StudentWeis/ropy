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

<p align="center">English | <a href="README.zh-CN.md">简体中文</a></p>

<p align="center">
<img src="https://raw.githubusercontent.com/StudentWeis/ropy-images/main/screenshots/2026-10-09/ropy-themes.png" alt="Ropy" width="80%">
</p>

## Features

- Native desktop application built with Zed's GPUI.
- Cross-platform support for Windows, macOS, and Linux (X11 and native [Wayland](docs/wayland.md)), with compositor-dependent clipboard and shortcut capabilities.
- Tracks text, rich text (HTML/RTF), image, and file path clipboard history with content-based deduplication.
- Switchable list and grid layout modes for browsing history.
- Search the loaded history with case-sensitive and whole-word options.
- Favorite and pin records; automatic cleanup preserves pinned and favorited items.
- Preview text and images via configurable hover delay or by holding `Space`.
- Configurable global hotkey, theme, language, autostart, and confirm mode.
- System tray integration plus in-app update check, download, and install flow.

## Installation

Download pre-built binaries from [Releases](https://github.com/StudentWeis/ropy/releases), or install via a package manager below.

### macOS

Install with the self-hosted [Homebrew tap](https://github.com/StudentWeis/homebrew-tap) (`brew upgrade --cask ropy` to update; updates are managed by Homebrew, while in-app update checks remain available):

```sh
brew install --cask studentweis/tap/ropy
```

For a manual DMG install, verify that you trust the download source and drag `Ropy.app` to Applications.

> [!NOTE]
> Current releases are unsigned and not notarized. The Homebrew Cask removes `com.apple.quarantine` automatically; for manual DMG installs, allow Ropy in **System Settings → Privacy & Security** or run:
>
> ```sh
> xattr -r -d com.apple.quarantine /Applications/Ropy.app
> ```
>
> Prefix with `sudo` if you see a permission error.

### Windows

Install with [Scoop](https://scoop.sh/) (`scoop update ropy` to update; Ropy detects standard Scoop installations and leaves updates to Scoop, while in-app update checks remain available):

```powershell
scoop bucket add extras
scoop install ropy
```

### Linux (X11 and Wayland)

- **Arch Linux**: Build `ropy-bin` using the [AUR packaging instructions](packaging/aur/README.md) (check publishing and validation steps there before assuming AUR availability).
- **Wayland**: Current source supports native Wayland without XWayland or `DISPLAY` (check release notes for pre-built binaries) and opens the window via `ropy --show`. Clipboard capture requires `data-control` and auto-paste requires `virtual-keyboard`; native Sway is tested, while GNOME, KDE, and Niri are unvalidated. See the [Wayland guide](docs/wayland.md) for activation, floating rules, and capability limits.

### Build from source

See [Development](docs/development.md) for prerequisites, locked source builds, Make targets, and optional Git hooks. Source builds reflect the current checkout rather than a specific release.

## Usage

- Launch Ropy — it starts hidden in the system tray and begins recording clipboard history.
- Use the global hotkey or the tray icon to open the history window.
- Press `/` to focus search, then refine results with case-sensitive, whole-word, and type filters.
- Use `Up`/`Down` or `J`/`K` to move through items, and `Enter` or `1`-`5` to confirm a selection.
- Press `Shift+Enter` to paste as plain text when a rich-text record is selected.
- In grid mode, use `H`/`L` to move between columns.
- Hold `Space` to preview the selected record, press `F` to favorite it, and `Delete` or `D` to remove it.
- Use row actions to pin records so they are excluded from storage cleanup.
- Choose between `copy_to_clipboard` and `paste_immediately` confirm modes in Settings.

History shortcuts can be [customized in Settings or Help](docs/keybindings.md), including disabling individual aliases such as `D` for deletion. Use [custom themes](docs/themes.md) to preview, duplicate and edit a palette without changing bundled themes.

## Documentation

Use the [documentation index](docs/README.md) to find user guides, development and testing commands, architecture, release acceptance and historical measurements. Start with [configuration and recovery](docs/configuration.md) for settings files or load errors, or [Contributing](CONTRIBUTING.md) to submit a change.

## Limitations and scope

- The system clipboard does not expose the original application source for copied items.
- Plugin/extension system: no current plans — Ropy focuses on simplicity and small footprint.
- Cloud sync: not supported at this time.
- Full command-line history management: Ropy is primarily a GUI application; Linux `ropy --show` opens that GUI.

## Acknowledgements

- Inspired by clipboard managers such as Ditto, Maccy and CopyQ.
- Thanks to the Rust community and all upstream projects used by Ropy: [GPUI Kit](https://github.com/longbridge/gpui-kit), [clipboard-rs](https://github.com/ChurchTao/clipboard-rs), [redb](https://github.com/cberner/redb) and others.

## Star History

<a href="https://www.star-history.com/?repos=studentweis%2Fropy&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=studentweis/ropy&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=studentweis/ropy&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=studentweis/ropy&type=date&legend=top-left" />
 </picture>
</a>

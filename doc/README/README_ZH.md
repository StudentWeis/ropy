<p align="center"><img src="../../assets/logo.png" alt="Ropy Logo" width="20%"></p>

<h2 align="center"><em><strong>R</strong>opy <strong>O</strong>rganizes <strong>P</strong>revious <strong>Y</strong>anks</em></h2>

<p align="center">
<a href="https://github.com/studentweis/ropy/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="License"></a>
<a href="https://github.com/studentweis/ropy/releases"><img src="https://img.shields.io/github/v/release/studentweis/ropy" alt="Release"></a>
<a href="https://rust-lang.org"><img src="https://img.shields.io/badge/language-Rust-orange" alt="Language"></a>
<br>
<a href="https://github.com/studentweis/ropy"><img src="https://img.shields.io/github/stars/studentweis/ropy?style=social" alt="Stars"></a>
<a href="https://github.com/studentweis/ropy/issues"><img src="https://img.shields.io/github/issues/studentweis/ropy" alt="Issues"></a>
</p>

<p align="center">使用 Rust 和 GPUI 编写的跨平台原生剪贴板管理器。</p>

<p align="center">
<a href="../../README.md">English</a> | 简体中文
</p>

<p align="center">
<img src="https://www.imgur.la/images/2026/04/02/pic_1775108102590.png" alt="Ropy Dark" width="80%">
</p>

## 特性

- 使用 Zed 的 GPUI 构建的原生桌面应用。
- 跨平台支持：Windows、macOS 和 Linux（X11）。
- 追踪文本、图片和文件路径剪贴板历史，并基于内容进行去重。
- 支持使用大小写敏感和整词匹配选项搜索已加载的历史记录。
- 支持收藏和置顶记录；自动清理时会保留已置顶和已收藏的内容。
- 支持预览文本和图片，并可配置悬浮预览延迟。
- 可配置全局快捷键、主题、语言、开机自启和确认模式。
- 提供系统托盘集成，以及应用内检查更新、下载和安装更新的流程。

## 安装

### 预编译二进制文件

您可以从 [Releases](https://github.com/StudentWeis/ropy/releases) 页面下载最新的预编译二进制文件。

### macOS

下载 `.dmg` 文件并将 Ropy.app 拖到应用程序文件夹后，您可能需要移除隔离属性才能正常运行应用程序。打开终端并运行以下命令：

```sh
xattr -rc /Applications/Ropy.app
sudo xattr -r -d com.apple.quarantine /Applications/Ropy.app
```

### Windows

您可以使用 [Scoop](https://scoop.sh/) 安装 Ropy：

```powershell
scoop install ropy
```

> [!NOTE]
> 如果您通过 Scoop 安装 Ropy，建议禁用 Ropy 内置的自动更新功能，以避免与 Scoop 的包管理产生冲突。您可以在 Ropy 的设置中禁用自动更新。

### 从源码构建

确保您已安装 Rust（通过 `rustup`）。然后：

```bash
git clone https://github.com/StudentWeis/ropy.git
cd ropy
cargo build --release
./target/release/ropy
```

## 使用

- 启动 Ropy 开始记录剪贴板历史。
- 使用全局快捷键或托盘图标打开历史窗口。
- 按 `/` 聚焦搜索，然后使用大小写敏感、整词匹配和类型筛选来优化结果。
- 使用 `Up`/`Down` 或 `J`/`K` 在条目间移动，按 `Enter` 或 `1`-`5` 确认选择。
- 按 `Space` 切换预览，按 `F` 收藏选中的记录，按 `Delete` 或 `D` 删除它。
- 使用行内操作置顶记录，使其排除在存储清理之外。
- 在设置中选择 `copy_to_clipboard` 或 `paste_immediately` 确认模式。

## 局限性（无计划支持）

- 系统剪贴板不会暴露复制条目的原始应用程序来源。
- 插件/扩展系统：目前没有计划 —— Ropy 专注于简洁和小体积。
- 云同步：目前不支持。
- 命令行模式：Ropy 主要设计为 GUI 应用程序。

## 致谢

- 灵感来自其他剪贴板管理器，如 Ditto、Maccy 和 CopyQ。
- 感谢 Rust 社区以及 Ropy 使用的所有上游项目。
- 系统剪贴板 API：[clipboard-rs](https://github.com/ChurchTao/clipboard-rs)
- GUI 库：[Zed's gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui)
- GUI 组件：[gpui-component](https://github.com/longbridge/gpui-component)
- 全局快捷键：[global-hotkey](https://github.com/tauri-apps/global-hotkey)
- 托盘图标辅助：[tray-icon](https://github.com/tauri-apps/tray-icon)
- 嵌入式数据库：[sled](https://github.com/spacejam/sled)
- 配置管理：[config-rs](https://github.com/rust-cli/config-rs)

## Star 历史

<a href="https://www.star-history.com/?repos=studentweis%2Fropy&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=studentweis/ropy&type=date&theme=dark&legend=top-left" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=studentweis/ropy&type=date&legend=top-left" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=studentweis/ropy&type=date&legend=top-left" />
 </picture>
</a>

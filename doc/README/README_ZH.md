<p align="center"><img src="../../assets/logo.png" alt="Ropy Logo" width="20%"></p>

<h2 align="center"><em><strong>R</strong>opy <strong>O</strong>rganizes <strong>P</strong>revious <strong>Y</strong>anks</em></h2>

<p align="center">
<a href="https://github.com/studentweis/ropy/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue" alt="License"></a>
<a href="https://github.com/studentweis/ropy/releases"><img src="https://img.shields.io/github/v/release/studentweis/ropy" alt="Release"></a>
<a href="https://rust-lang.org"><img src="https://img.shields.io/badge/language-Rust-orange" alt="Language"></a>
</br>
<a href="https://github.com/studentweis/ropy"><img src="https://img.shields.io/github/stars/studentweis/ropy?style=social" alt="Stars"></a>
<a href="https://github.com/studentweis/ropy/issues"><img src="https://img.shields.io/github/issues/studentweis/ropy" alt="Issues"></a>
</p>

<p align="center">一个使用纯 Rust 编写的跨平台原生剪贴板管理器。</p>

<p align="center">
<a href="../../README.md">English</a> | 简体中文
</p>

<p align="center">
<img src="https://www.imgur.la/images/2026/03/13/pic_1773332541063.png" alt="Ropy Dark" width="40%">
<img src="https://www.imgur.la/images/2026/03/13/pic_1773332561411.png" alt="Ropy Light" width="40%">
</p>

## 特性

- 跨平台支持：Windows、macOS 和 Linux(X11)。
- 使用 Zed 的 GPUI 构建的原生 GUI 应用。
- 易于使用、轻量且快速。
- 搜索/置顶/预览/自动启动/快捷键/自动更新/记录置顶。

## 安装

### 预编译二进制文件

您可以从 [Releases](https://github.com/StudentWeis/ropy/releases) 页面下载最新的预编译二进制文件。

### macOS

下载 `.dmg` 文件并将 Ropy.app 拖到应用程序文件夹后，您可能需要移除隔离属性才能正常运行应用程序。打开终端并运行以下命令：

```sh
xattr -rc /Applications/Ropy.app
sudo xattr -r -d com.apple.quarantine /Applications/Ropy.app
```

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
- 点击记录或按数字键（`1`-`5`）或 `Enter` 进行粘贴。
- 按 `Space` 预览条目而不粘贴。
- 使用搜索栏按内容筛选记录。
- 置顶条目以防止在达到历史限制时被清理。

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

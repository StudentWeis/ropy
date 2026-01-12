<p align="center"><img src="../assets/logo.png" alt="Ropy Logo" width="20%"></p>

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
<a href="../README.md">English</a> | 简体中文
</p>

<p align="center">
<img src="https://s2.loli.net/2026/01/04/8LNOkP3rwivYtxj.png" alt="Ropy Dark" width="40%">
<img src="https://s2.loli.net/2026/01/04/IM83wFmDJ6QpKHO.png" alt="Ropy Light" width="40%">
</p>

# 特性

- 跨平台支持：Windows、macOS 和 Linux(X11)。
- 使用 Zed 的 GPUI 构建的原生 GUI 应用。
- 易于使用、轻量且快速。
- 搜索/置顶/预览/自动启动/快捷键。

# 安装

## 预编译二进制文件

您可以从 [Releases](https://github.com/StudentWeis/ropy/releases) 页面下载最新的预编译二进制文件。

### macOS

下载 `.dmg` 文件并将 Ropy.app 拖到应用程序文件夹后，您可能需要移除隔离属性才能正常运行应用程序。打开终端并运行以下命令：

```sh
xattr -rc /Applications/Ropy.app
sudo xattr -r -d com.apple.quarantine /Applications/Ropy.app
```

## 从源码构建

确保您已安装 Rust。您可以使用 [rustup](https://rustup.rs/) 安装 Rust。

1. 克隆仓库：

```bash
git clone https://github.com/StudentWeis/ropy.git
cd ropy
```

2. 构建项目：

```bash
cargo build --release
```

3. 运行应用：

```bash
./target/release/ropy
```

# 使用

- 启动应用程序，它将开始监控您的剪贴板。
- 使用可配置的全局快捷键或托盘图标访问剪贴板历史记录。
- 点击任意记录或使用 <kbd>1/2/3/4/5</kbd> 或 <kbd>Enter</kbd> 选择记录。
- 使用搜索栏筛选剪贴板记录。
- 置顶 Ropy 窗口以使其始终保持在最上层。

# 致谢

- 灵感来自其他剪贴板管理器，如 Ditto、Maccy 和 CopyQ。
- 感谢 Rust 社区的支持和库。
- 系统剪贴板 API：[clipboard-rs](https://github.com/ChurchTao/clipboard-rs)
- GUI 库：[Zed's gpui](https://github.com/zed-industries/zed/tree/main/crates/gpui)
- GUI 组件：[gpui-component](https://github.com/longbridge/gpui-component)
- 全局快捷键：[global-hotkey](https://github.com/tauri-apps/global-hotkey)
- 托盘图标：[tray-icon](https://github.com/tauri-apps/tray-icon)
- 数据库：[sled](https://github.com/spacejam/sled)
- 配置管理：[config-rs](https://github.com/rust-cli/config-rs)

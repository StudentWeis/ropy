# 贡献指南

[English](../../CONTRIBUTING.md) | [简体中文](README.md)

感谢你考虑为 Ropy 做贡献！本文档将指导你如何为项目添加新的主题和多语言支持。

## 添加新主题

Ropy 使用 TOML 格式的配置文件来定义主题。主题文件位于 `assets/themes/` 目录下。

### 步骤

1. **创建主题文件**

   在 `assets/themes/` 目录下创建一个新的 `.toml` 文件，文件名即为主题的 ID（使用小写字母和连字符，例如 `my-theme.toml`）。

2. **定义主题内容**

   主题文件需要包含以下字段：

   ```toml
   # 主题显示名称
   theme_name = "My Theme"

   # 主题模式：light 或 dark
   mode = "dark"

   # 以下是颜色定义（使用十六进制颜色值）
   background = "#2d2d2d"           # 主背景色
   foreground = "#ffffff"           # 主前景色（文字颜色）
   secondary = "#3d3d3d"            # 次要背景色
   secondary_foreground = "#ffffff" # 次要前景色
   border = "#4d4d4d"               # 边框颜色
   accent = "#4d4d4d"               # 强调色背景
   accent_foreground = "#ffffff"    # 强调色前景
   muted = "#3d3d3d"                # 静音背景色
   muted_foreground = "#888888"     # 静音前景色
   input = "#555555"                # 输入框背景色
   primary = "#6b8cff"              # 主色调
   primary_foreground = "#ffffff"   # 主色调前景
   primary_hover = "#5a7ae6"        # 主色调悬停状态
   primary_active = "#4a68cc"       # 主色调激活状态
   danger = "#f56565"               # 危险/错误色
   danger_foreground = "#ffffff"    # 危险色前景
   popover = "#353535"              # 弹出层背景色
   popover_foreground = "#ffffff"   # 弹出层前景色
   selection = "#46506a"            # 选中区域颜色
   ring = "#6b8cff"                 # 焦点环颜色
   list_hover = "#3d3d3d"           # 列表项悬停背景色
   list_active = "#4d4d4d"          # 列表项激活背景色
   scrollbar_thumb = "#555555"      # 滚动条滑块颜色
   ```

3. **重新编译项目**

   主题文件在编译时会被嵌入到二进制文件中，因此需要重新编译：

   ```bash
   cargo build --release
   ```

### 示例

参考现有的主题文件：
- `assets/themes/ropy-dark.toml` - 深色主题示例
- `assets/themes/ropy-light.toml` - 浅色主题示例
- `assets/themes/nord-light.toml` - Nord 配色方案示例
- `assets/themes/everforest-night.toml` - Everforest 配色方案示例

## 添加新语言

Ropy 支持多语言国际化（i18n）。语言文件同样使用 TOML 格式，位于 `assets/locales/` 目录下。

### 步骤

1. **创建语言文件**

   在 `assets/locales/` 目录下创建一个新的 `.toml` 文件，文件名使用语言代码（例如 `fr.toml` 表示法语，`ko.toml` 表示韩语）。

   语言代码应遵循 [BCP 47](https://tools.ietf.org/html/bcp47) 标准：
   - 对于区域变体，使用 `{语言代码}-{区域代码}` 格式，例如：
     - `zh-CN` - 简体中文
     - `zh-TW` - 繁体中文（台湾）
     - `pt-BR` - 巴西葡萄牙语

2. **定义语言内容**

   语言文件的第一行应包含 `language_name` 字段，用于在设置中显示：

   ```toml
   # 语言显示名称（用该语言本身书写）
   language_name = "Français"

   # 以下是翻译键值对
   # 托盘菜单
   tray_show = "Afficher"
   tray_quit = "Quitter"

   # 主窗口
   clear_all = "Tout effacer"
   clear_confirm_title = "Effacer tous les enregistrements"
   clear_confirm_message = "Cela supprimera définitivement tous les enregistrements du presse-papiers. Cette action est irréversible."
   clear_confirm_cancel = "Annuler"
   clear_confirm_button = "Effacer"

   # ... 其他翻译键
   ```

3. **完整的翻译键列表**

   参考 `assets/locales/en.toml` 获取所有需要翻译的键。以下是主要分类：

   - **托盘菜单** (`tray_show`, `tray_quit`)
   - **主窗口** (`clear_all`, `pin`, `unpin`, `filter_*`, `search_*`)
   - **设置** (`settings_*`)
   - **关于** (`about_*`)
   - **更新** (`update_*`)
   - **键盘快捷键** (`help_*`)

4. **重新编译项目**

   语言文件同样在编译时嵌入，需要重新编译：

   ```bash
   cargo build --release
   ```

### 翻译技巧

1. **保持简洁**：UI 空间有限，尽量使用简洁的表达
2. **保持一致性**：相同的概念使用相同的翻译
3. **尊重用户习惯**：使用目标语言用户的习惯表达方式
4. **测试显示效果**：启动应用验证翻译在实际界面中的显示效果

### 示例

参考现有的语言文件：
- `assets/locales/en.toml` - 英语（基准语言）
- `assets/locales/zh-CN.toml` - 简体中文
- `assets/locales/ja.toml` - 日语

## 代码风格

- 遵循 [Clean Code](https://www.oreilly.com/library/view/clean-code-a/9780136083238/) 原则
- 保持代码简洁（KISS）和避免重复（DRY）
- 使用 `thiserror` 定义错误类型
- 在提交代码前，运行检查脚本：

   ```bash
   ./scripts/precheck.sh
   ```

## 测试

查看 [测试文档](../TESTING.md) 了解如何编写和运行测试。

## 提交规范

1. Fork 本仓库
2. 创建特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 问题反馈

如果你发现 bug 或有功能建议，请在 [Issues](https://github.com/StudentWeis/ropy/issues) 页面提交。

感谢你的贡献！

---

**其他语言**: [English](../../CONTRIBUTING.md)

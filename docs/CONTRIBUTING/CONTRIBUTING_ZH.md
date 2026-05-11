# 贡献指南

[English](../../CONTRIBUTING.md) | 简体中文

感谢你考虑为 Ropy 做贡献！本文档将指导你如何为项目添加新的主题和多语言支持。

## 添加新主题

Ropy 使用 TOML 格式的配置文件来定义主题。主题文件位于 `assets/themes/` 目录下。

### 步骤

1. **创建主题文件**

   在 `assets/themes/` 目录下创建一个新的 `.toml` 文件，文件名即为主题的 ID（使用小写字母和连字符，例如 `my-theme.toml`）。

2. **定义主题内容**

   主题文件需要包含 `theme_name`、`mode`（`light` 或 `dark`）以及完整的颜色字段集（十六进制颜色值）。完整的字段列表和格式请参考 [`assets/themes/ropy-dark.toml`](../../assets/themes/ropy-dark.toml)。

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

   语言文件的第一行应包含 `language_name` 字段（用该语言本身书写），用于在设置中显示。完整的结构和所有翻译键请参考 [`assets/locales/en.toml`](../../assets/locales/en.toml)。

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

## 测试

查看 [测试文档](../TESTING.md) 了解如何编写和运行测试。

## 提交规范

Ropy 采用 **Issue → 分支 → PR → Squash Merge** 的工作流，全程通过本地 `gh` CLI 完成。完整、可直接复制执行的 SOP 收敛在 [`contribution-flow`](../../.agents/skills/contribution-flow/SKILL.md) skill 中 —— 那里是分支命名、commit 规范、`scripts/precheck.sh` 检查、PR 自检清单的唯一权威来源。无论人类还是 AI 贡献者，都请遵循该 skill。

## 问题反馈

如果你发现 bug 或有功能建议，请在 [Issues](https://github.com/StudentWeis/ropy/issues) 页面选择对应模板提交。

感谢你的贡献！

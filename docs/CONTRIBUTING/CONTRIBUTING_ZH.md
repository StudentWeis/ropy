# 贡献指南

[English](../../CONTRIBUTING.md) | 简体中文

本文说明如何贡献代码、内置主题、翻译和文档。如果只想创建自己使用的配色，请按[自定义主题](../THEMES.md)操作，无需提交内置资源。

## 准备修改

1. 按照 [contribution-flow](../../.agents/skills/contribution-flow/SKILL.md) 执行 Issue → 分支 → PR → Squash Merge。模板、分支与提交命名、precheck 和合并授权以该流程为准。
2. 按[开发指南](../DEVELOPMENT.md)准备工具链和原生依赖。代码规范见 [AGENTS.md](../../AGENTS.md)，模块边界见[架构](../ARCHITECTURE.md)。
3. 按[测试指南](../TESTING.md)选择测试。提交前必须执行 `./scripts/precheck.sh`，单独的资源检查不能替代完整门禁。

## 添加内置主题

内置主题位于 [assets/themes](../../assets/themes/)。新建完整的 `.toml` 文件，使用小写字母和连字符命名，例如 `my-theme.toml`；不含扩展名的文件名是内置主题 ID。

从 [ropy-dark.toml](../../assets/themes/ropy-dark.toml) 或 [ropy-light.toml](../../assets/themes/ropy-light.toml) 开始，保留非空的 `theme_name`、`mode`（`light` 或 `dark`）及全部颜色字段。颜色使用六位十六进制 RGB 值，精确字段以模板为准，不另外维护一份字段全集。

执行 `python3 scripts/check/check_themes.py`，然后使用隔离的应用构建检查可读性、选中与悬浮状态，以及列表和网格布局。资源检查不能证明视觉质量；界面修改还须遵守 [GPUI 技能](../../AGENTS.md#gpui-skills)。

## 添加或更新语言

语言文件位于 [assets/locales](../../assets/locales/)，使用 TOML 格式。以 [en.toml](../../assets/locales/en.toml) 的结构和键为准，文件名使用 [BCP 47](https://www.rfc-editor.org/rfc/bcp/bcp47.txt) 语言代码，例如 `fr.toml`、`ko.toml` 或 `pt-BR.toml`。

将 `language_name` 设为该语言自身的名称，供设置选择器显示。翻译保持简洁、术语一致，并保留 `%{count}` 等插值参数名；重新构建后检查实际界面。资源在编译期嵌入，添加语言文件不需要额外维护语言列表。

应用直接调用 `rust_i18n::t!`，缺失翻译回退到英语。动态翻译键使用以 `_key` 结尾的字段保存字面量，供检查脚本识别。仅启动入口及语言设置保存成功后设置共享 locale；测试和离线夹具使用显式 locale。不要新增翻译映射或语言全局变量。

运行 `python3 scripts/check/check_i18n.py` 检查 TOML、键和插值参数一致性。运行时本地化职责见[架构](../ARCHITECTURE.md#dependency-direction)。

## 维护文档

编写或整理项目文档前，阅读 [project-doc](../../.agents/skills/project-doc/SKILL.md)。通过[文档索引](../README.md)选择已有的事实拥有者，并同步中英文 README 和贡献入口。不要手改生成的基准结果，也不要将历史测量写成当前版本已经完成的验证。

## 发布与问题反馈

维护者按照[发布流程](../RELEASING.md#protected-release-sequence)准备版本和发布，按照[发布包验收](../RELEASE_ACCEPTANCE.md)记录精确制品的 GUI 证据。整理文档或创建 PR 不代表获准发布或合并。

报告缺陷或功能建议时，选择对应的 [Issue 模板](https://github.com/StudentWeis/ropy/issues/new/choose)，提供平台、版本、复现步骤和相关日志，不附带个人剪贴板内容。

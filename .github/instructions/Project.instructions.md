---
applyTo: "**"
---

- 最好使用 gpui-component 来添加新的组件。
- 最好使用 thiserror 来定义错误类型。
- 单测中的 unwrap() 和 expect() 是被允许的，可以在单测函数上添加 #[allow(clippy::unwrap_used)]、#[allow(clippy::expect_used)] 来避免 clippy 警告。
- 使用 script/precheck.sh 来检查代码质量和格式。

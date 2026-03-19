开发相关：

- 注释应该是通用的、规范的、简洁的。
- 遵循 TDD（测试驱动开发）的思路，优先编写测试，再编写实现代码。
- 使用 context7 查询库文档。
- 最好使用 thiserror 来定义错误类型。
- 最好使用 gpui-component 来添加新的组件。
- 使用 script/precheck.sh 来检查代码质量和格式。

测试相关：

- 如使用 unwrap() 和 expect()，需添加 #[allow(clippy::unwrap_used)]、#[allow(clippy::expect_used)] 来避免 clippy 警告。
- 使用 tempfile 库创建临时文件。
- 使用 rstest 进行参数化测试。
- 单测统一使用 `test_<对象>_<场景>_<预期>` 命名模式。

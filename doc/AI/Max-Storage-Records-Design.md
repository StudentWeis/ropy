
# 存储上限与显示上限分离设计方案

## 一、背景

当前 `max_history_records` 配置项同时承担两个职责：

1. **存储上限**：决定 `repo.cleanup_old_records()` 时保留的最大记录数
2. **显示上限**：决定 UI 面板中显示的最大记录数

这导致用户无法单独控制「后台存储多少条」和「界面显示多少条」，缺乏灵活性。

## 二、解决方案

### 2.1 新增配置项

在 `StorageSettings` 中新增 `max_storage_records` 字段：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageSettings {
    /// Maximum number of records to display in the UI (1 - 10,000)
    pub max_history_records: usize,
    /// Maximum number of records to store in the repository (1 - 100,000)
    /// Must be >= max_history_records
    pub max_storage_records: usize,
}
```

### 2.2 默认值

| 配置项 | 默认值 | 说明 |
|--------|--------|------|
| `max_history_records` | 100 | UI 显示数量（保持现有默认值） |
| `max_storage_records` | 1000 | 存储数量，比显示数量大一个数量级 |

### 2.3 配置文件示例

用户 `config.toml` 示例：

```toml
[storage]
max_history_records = 100    # UI 显示 100 条
max_storage_records = 5000   # 后台存储 5000 条
```

## 三、修改文件清单

| 文件 | 修改内容 |
|------|----------|
| `src/config/settings.rs` | 在 `StorageSettings` 中添加 `max_storage_records` 字段 |
| `src/app.rs` | `start_clipboard_event_handler` 使用 `max_storage_records` 作为 `cleanup_old_records` 的参数；`load_initial_records` 继续使用 `max_history_records` |
| `src/gui/panel/settings.rs` | 添加 `max_storage_records` 的输入框 UI |
| `src/gui/board/mod.rs` | 添加 `settings_max_storage_input` 字段和验证逻辑 |
| `assets/locales/zh-CN.toml` | 添加 `settings_max_storage` 翻译 |
| `assets/locales/en.toml` | 添加 `settings_max_storage` 翻译 |
| `assets/locales/ja.toml` | 添加 `settings_max_storage` 翻译 |

## 四、关键逻辑变化

### 4.1 存储清理逻辑 (`app.rs`)

**现有逻辑（单一配置）：**

```rust
let max_history_records = settings_guard.storage.max_history_records;
guard.truncate(max_history_records);
repo.cleanup_old_records(max_history_records).ok();
```

**新逻辑（双配置）：**

```rust
let max_storage_records = settings_guard.storage.max_storage_records;
// 内存中保留显示数量
let max_display = settings_guard.storage.max_history_records;
guard.truncate(max_display);
// 持久化存储保留更大数量
repo.cleanup_old_records(max_storage_records).ok();
```

### 4.2 应用启动时加载 (`app.rs` — `load_initial_records`)

保持不变，继续使用 `max_history_records` 加载初始记录：

```rust
let max_records = settings_guard.storage.max_history_records;
repository.and_then(|repo| repo.get_recent(max_records).ok())
```

### 4.3 设置验证逻辑

```rust
// 解析验证时需确保 max_storage_records >= max_history_records
fn parse_storage_settings(...) {
    let display = parse_display_limit()?;
    let storage = parse_storage_limit()?;
    if storage < display {
        return Err("max_storage_records must >= max_history_records");
    }
    Ok((display, storage))
}
```

## 五、边界情况处理

| 场景 | 处理方式 |
|------|----------|
| `max_storage < max_history` | 保存设置时报错，或自动调整 `max_storage = max_history` |
| 用户修改 `max_history` 超过 `max_storage` | 自动同步提升 `max_storage` |
| 现有用户升级 | `max_storage_records` 未设置时使用默认值 1000 |

## 六、可选增强

1. **智能提示**：在设置面板中显示当前实际存储数量，帮助用户判断是否需要调整
2. **批量清理**：当用户减小 `max_storage_records` 时，立即触发清理
3. **性能优化**：大存储量 + 小显示量时，内存中只缓存 `max_history_records` 数量

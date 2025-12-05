//! 剪切板历史记录存储模块
//!
//! 使用 sled 嵌入式数据库持久化存储剪切板历史记录。
//! 支持文本内容的存储、查询和删除操作。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sled::{Db, Tree};
use std::path::PathBuf;

/// 剪切板记录的数据模型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClipboardRecord {
    /// 唯一标识符（时间戳纳秒）
    pub id: u64,
    /// 剪切板内容
    pub content: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 内容类型
    pub content_type: ContentType,
}

/// 内容类型枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContentType {
    /// 纯文本
    Text,
    /// 图片（存储为 base64）
    Image,
    /// 文件路径
    FilePath,
}

/// 剪切板历史记录仓库
pub struct ClipboardRepository {
    db: Db,
    records_tree: Tree,
}

impl ClipboardRepository {
    /// 创建新的仓库实例
    ///
    /// 数据库文件存储在用户数据目录下的 `ropy/clipboard.db`
    pub fn new() -> Result<Self, RepositoryError> {
        let data_dir = Self::get_data_dir()?;
        Self::with_path(data_dir)
    }

    /// 使用指定路径创建仓库实例（用于测试）
    pub fn with_path(path: PathBuf) -> Result<Self, RepositoryError> {
        let db = sled::open(&path).map_err(|e| RepositoryError::DatabaseOpen(e.to_string()))?;
        let records_tree = db
            .open_tree("clipboard_records")
            .map_err(|e| RepositoryError::TreeOpen(e.to_string()))?;

        Ok(Self { db, records_tree })
    }

    /// 获取数据目录路径
    fn get_data_dir() -> Result<PathBuf, RepositoryError> {
        let data_dir = dirs::data_local_dir()
            .ok_or(RepositoryError::DataDirNotFound)?
            .join("ropy")
            .join("clipboard.db");
        Ok(data_dir)
    }

    /// 保存剪切板记录
    ///
    /// 使用时间戳作为 key，保证按时间顺序存储
    pub fn save(
        &self,
        content: String,
        content_type: ContentType,
    ) -> Result<ClipboardRecord, RepositoryError> {
        let now = Utc::now();
        let id = now.timestamp_nanos_opt().unwrap_or(0) as u64;

        let record = ClipboardRecord {
            id,
            content,
            created_at: now,
            content_type,
        };

        let key = id.to_be_bytes();
        let value = serde_json::to_vec(&record)
            .map_err(|e| RepositoryError::Serialization(e.to_string()))?;

        self.records_tree
            .insert(key, value)
            .map_err(|e| RepositoryError::Insert(e.to_string()))?;

        Ok(record)
    }

    /// 保存文本内容（便捷方法）
    pub fn save_text(&self, content: String) -> Result<ClipboardRecord, RepositoryError> {
        self.save(content, ContentType::Text)
    }

    /// 检查内容是否与最后一条记录重复
    pub fn is_duplicate(&self, content: &str) -> Result<bool, RepositoryError> {
        if let Some(last_record) = self.get_latest()? {
            return Ok(last_record.content == content);
        }
        Ok(false)
    }

    /// 保存文本（带去重检查）
    pub fn save_text_if_not_duplicate(
        &self,
        content: String,
    ) -> Result<Option<ClipboardRecord>, RepositoryError> {
        if self.is_duplicate(&content)? {
            return Ok(None);
        }
        self.save_text(content).map(Some)
    }

    /// 获取最新的一条记录
    pub fn get_latest(&self) -> Result<Option<ClipboardRecord>, RepositoryError> {
        if let Some(result) = self
            .records_tree
            .last()
            .map_err(|e| RepositoryError::Query(e.to_string()))?
        {
            let (_, value) = result;
            let record: ClipboardRecord = serde_json::from_slice(&value)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            return Ok(Some(record));
        }
        Ok(None)
    }

    /// 根据 ID 获取记录
    pub fn get_by_id(&self, id: u64) -> Result<Option<ClipboardRecord>, RepositoryError> {
        let key = id.to_be_bytes();
        if let Some(value) = self
            .records_tree
            .get(key)
            .map_err(|e| RepositoryError::Query(e.to_string()))?
        {
            let record: ClipboardRecord = serde_json::from_slice(&value)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            return Ok(Some(record));
        }
        Ok(None)
    }

    /// 获取所有记录（按时间倒序）
    pub fn get_all(&self) -> Result<Vec<ClipboardRecord>, RepositoryError> {
        let mut records = Vec::new();
        for result in self.records_tree.iter().rev() {
            let (_, value) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
            let record: ClipboardRecord = serde_json::from_slice(&value)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            records.push(record);
        }
        Ok(records)
    }

    /// 获取最近 N 条记录（按时间倒序）
    pub fn get_recent(&self, limit: usize) -> Result<Vec<ClipboardRecord>, RepositoryError> {
        let mut records = Vec::new();
        for result in self.records_tree.iter().rev().take(limit) {
            let (_, value) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
            let record: ClipboardRecord = serde_json::from_slice(&value)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            records.push(record);
        }
        Ok(records)
    }

    /// 根据关键字搜索记录
    pub fn search(&self, keyword: &str) -> Result<Vec<ClipboardRecord>, RepositoryError> {
        let keyword_lower = keyword.to_lowercase();
        let mut records = Vec::new();
        for result in self.records_tree.iter().rev() {
            let (_, value) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
            let record: ClipboardRecord = serde_json::from_slice(&value)
                .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
            if record.content.to_lowercase().contains(&keyword_lower) {
                records.push(record);
            }
        }
        Ok(records)
    }

    /// 删除记录
    pub fn delete(&self, id: u64) -> Result<bool, RepositoryError> {
        let key = id.to_be_bytes();
        let removed = self
            .records_tree
            .remove(key)
            .map_err(|e| RepositoryError::Delete(e.to_string()))?;
        Ok(removed.is_some())
    }

    /// 清空所有记录
    pub fn clear(&self) -> Result<(), RepositoryError> {
        self.records_tree
            .clear()
            .map_err(|e| RepositoryError::Delete(e.to_string()))?;
        Ok(())
    }

    /// 获取记录总数
    ///
    /// 返回存储库中剪切板记录的总数。
    pub fn count(&self) -> usize {
        self.records_tree.len()
    }

    /// 刷新数据到磁盘
    pub fn flush(&self) -> Result<(), RepositoryError> {
        self.db
            .flush()
            .map_err(|e| RepositoryError::Flush(e.to_string()))?;
        Ok(())
    }

    /// 清理旧记录，保留最近 N 条
    pub fn cleanup_old_records(&self, keep_count: usize) -> Result<usize, RepositoryError> {
        let total = self.count();
        if total <= keep_count {
            return Ok(0);
        }

        let to_remove = total - keep_count;
        let mut removed = 0;

        for result in self.records_tree.iter().take(to_remove) {
            let (key, _) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
            self.records_tree
                .remove(key)
                .map_err(|e| RepositoryError::Delete(e.to_string()))?;
            removed += 1;
        }

        Ok(removed)
    }
}

/// 仓库错误类型
#[derive(Debug)]
pub enum RepositoryError {
    /// 数据目录未找到
    DataDirNotFound,
    /// 数据库打开失败
    DatabaseOpen(String),
    /// Tree 打开失败
    TreeOpen(String),
    /// 序列化错误
    Serialization(String),
    /// 反序列化错误
    Deserialization(String),
    /// 插入错误
    Insert(String),
    /// 查询错误
    Query(String),
    /// 删除错误
    Delete(String),
    /// 刷新错误
    Flush(String),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepositoryError::DataDirNotFound => write!(f, "无法找到数据目录"),
            RepositoryError::DatabaseOpen(e) => write!(f, "数据库打开失败: {}", e),
            RepositoryError::TreeOpen(e) => write!(f, "Tree 打开失败: {}", e),
            RepositoryError::Serialization(e) => write!(f, "序列化错误: {}", e),
            RepositoryError::Deserialization(e) => write!(f, "反序列化错误: {}", e),
            RepositoryError::Insert(e) => write!(f, "插入错误: {}", e),
            RepositoryError::Query(e) => write!(f, "查询错误: {}", e),
            RepositoryError::Delete(e) => write!(f, "删除错误: {}", e),
            RepositoryError::Flush(e) => write!(f, "刷新错误: {}", e),
        }
    }
}

impl std::error::Error for RepositoryError {}

impl Drop for ClipboardRepository {
    fn drop(&mut self) {
        self.flush().ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;

    fn create_test_repo() -> ClipboardRepository {
        let temp_dir = tempdir().expect("创建临时目录失败");
        let db_path = temp_dir.path().join("test.db");
        ClipboardRepository::with_path(db_path).expect("创建测试仓库失败")
    }

    #[test]
    fn test_save_and_get_text() {
        let repo = create_test_repo();

        let record = repo
            .save_text("Hello, World!".to_string())
            .expect("保存失败");
        assert_eq!(record.content, "Hello, World!");
        assert_eq!(record.content_type, ContentType::Text);

        let retrieved = repo
            .get_by_id(record.id)
            .expect("查询失败")
            .expect("记录不存在");
        assert_eq!(retrieved.content, "Hello, World!");
    }

    #[test]
    fn test_get_latest() {
        let repo = create_test_repo();

        repo.save_text("First".to_string()).expect("保存失败");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("Second".to_string()).expect("保存失败");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("Third".to_string()).expect("保存失败");

        let latest = repo.get_latest().expect("查询失败").expect("记录不存在");
        assert_eq!(latest.content, "Third");
    }

    #[test]
    fn test_duplicate_check() {
        let repo = create_test_repo();

        repo.save_text("Same content".to_string())
            .expect("保存失败");

        assert!(repo.is_duplicate("Same content").expect("检查失败"));
        assert!(!repo.is_duplicate("Different content").expect("检查失败"));
    }

    #[test]
    fn test_save_if_not_duplicate() {
        let repo = create_test_repo();

        let first = repo
            .save_text_if_not_duplicate("Content".to_string())
            .expect("保存失败");
        assert!(first.is_some());

        let second = repo
            .save_text_if_not_duplicate("Content".to_string())
            .expect("保存失败");
        assert!(second.is_none());

        let third = repo
            .save_text_if_not_duplicate("New Content".to_string())
            .expect("保存失败");
        assert!(third.is_some());

        assert_eq!(repo.count(), 2);
    }

    #[test]
    fn test_get_recent() {
        let repo = create_test_repo();

        for i in 1..=5 {
            repo.save_text(format!("Record {}", i)).expect("保存失败");
            thread::sleep(Duration::from_millis(10));
        }

        let recent = repo.get_recent(3).expect("查询失败");
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].content, "Record 5");
        assert_eq!(recent[1].content, "Record 4");
        assert_eq!(recent[2].content, "Record 3");
    }

    #[test]
    fn test_search() {
        let repo = create_test_repo();

        repo.save_text("Hello World".to_string()).expect("保存失败");
        repo.save_text("Goodbye World".to_string())
            .expect("保存失败");
        repo.save_text("Hello Rust".to_string()).expect("保存失败");

        let results = repo.search("hello").expect("搜索失败");
        assert_eq!(results.len(), 2);

        let results = repo.search("world").expect("搜索失败");
        assert_eq!(results.len(), 2);

        let results = repo.search("rust").expect("搜索失败");
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_delete() {
        let repo = create_test_repo();

        let record = repo
            .save_text("To be deleted".to_string())
            .expect("保存失败");
        assert_eq!(repo.count(), 1);

        let deleted = repo.delete(record.id).expect("删除失败");
        assert!(deleted);
        assert_eq!(repo.count(), 0);

        let deleted_again = repo.delete(record.id).expect("删除失败");
        assert!(!deleted_again);
    }

    #[test]
    fn test_clear() {
        let repo = create_test_repo();

        repo.save_text("One".to_string()).expect("保存失败");
        repo.save_text("Two".to_string()).expect("保存失败");
        repo.save_text("Three".to_string()).expect("保存失败");
        assert_eq!(repo.count(), 3);

        repo.clear().expect("清空失败");
        assert_eq!(repo.count(), 0);
    }

    #[test]
    fn test_cleanup_old_records() {
        let repo = create_test_repo();

        for i in 1..=10 {
            repo.save_text(format!("Record {}", i)).expect("保存失败");
            thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(repo.count(), 10);

        let removed = repo.cleanup_old_records(5).expect("清理失败");
        assert_eq!(removed, 5);
        assert_eq!(repo.count(), 5);

        // 验证保留的是最新的记录
        let recent = repo.get_recent(5).expect("查询失败");
        assert_eq!(recent[0].content, "Record 10");
        assert_eq!(recent[4].content, "Record 6");
    }

    #[test]
    fn test_get_all() {
        let repo = create_test_repo();

        repo.save_text("First".to_string()).expect("保存失败");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("Second".to_string()).expect("保存失败");
        thread::sleep(Duration::from_millis(10));
        repo.save_text("Third".to_string()).expect("保存失败");

        let all = repo.get_all().expect("查询失败");
        assert_eq!(all.len(), 3);
        // 按时间倒序
        assert_eq!(all[0].content, "Third");
        assert_eq!(all[1].content, "Second");
        assert_eq!(all[2].content, "First");
    }
}

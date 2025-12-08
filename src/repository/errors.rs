//! 仓库错误类型

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

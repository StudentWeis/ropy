//! Repository error types

#[derive(Debug)]
pub enum RepositoryError {
    /// Data directory not found
    DataDirNotFound,
    /// Database open failed
    DatabaseOpen(String),
    /// Tree open failed
    TreeOpen(String),
    /// Serialization error
    Serialization(String),
    /// Deserialization error
    Deserialization(String),
    /// Insert error
    Insert(String),
    /// Query error
    Query(String),
    /// Delete error
    Delete(String),
    /// Flush error
    Flush(String),
}

impl std::fmt::Display for RepositoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DataDirNotFound => write!(f, "Data directory not found"),
            Self::DatabaseOpen(e) => write!(f, "Database open failed: {e}"),
            Self::TreeOpen(e) => write!(f, "Tree open failed: {e}"),
            Self::Serialization(e) => write!(f, "Serialization error: {e}"),
            Self::Deserialization(e) => write!(f, "Deserialization error: {e}"),
            Self::Insert(e) => write!(f, "Insert error: {e}"),
            Self::Query(e) => write!(f, "Query error: {e}"),
            Self::Delete(e) => write!(f, "Delete error: {e}"),
            Self::Flush(e) => write!(f, "Flush error: {e}"),
        }
    }
}

impl std::error::Error for RepositoryError {}

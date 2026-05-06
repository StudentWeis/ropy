//! Repository error types.

#[derive(Debug, thiserror::Error)]
pub(crate) enum RepositoryError {
    #[error("Data directory not found")]
    DataDirNotFound,
    #[error("Database open failed: {0}")]
    DatabaseOpen(String),
    #[error("Tree open failed: {0}")]
    TreeOpen(String),
    #[error("Serialization error: {0}")]
    Serialization(String),
    #[error("Deserialization error: {0}")]
    Deserialization(String),
    #[error("Insert error: {0}")]
    Insert(String),
    #[error("Query error: {0}")]
    Query(String),
    #[error("Delete error: {0}")]
    Delete(String),
}

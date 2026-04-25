//! Repository error types

/// Errors that can occur during repository operations.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    /// Data directory not found
    #[error("Data directory not found")]
    DataDirNotFound,
    /// Database open failed
    #[error("Database open failed: {0}")]
    DatabaseOpen(String),
    /// Tree open failed
    #[error("Tree open failed: {0}")]
    TreeOpen(String),
    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(String),
    /// Deserialization error
    #[error("Deserialization error: {0}")]
    Deserialization(String),
    /// Insert error
    #[error("Insert error: {0}")]
    Insert(String),
    /// Query error
    #[error("Query error: {0}")]
    Query(String),
    /// Delete error
    #[error("Delete error: {0}")]
    Delete(String),
}

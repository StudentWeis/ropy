//! Repository error types.

/// Failure to open, query or mutate clipboard history.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    /// An image or rich-text payload file could not be persisted.
    #[error("Payload sidecar write failed: {0}")]
    Sidecar(#[from] std::io::Error),
    /// The platform has no usable application data directory.
    #[error("Data directory not found")]
    DataDirNotFound,
    /// The underlying database could not be opened.
    #[error("Database open failed: {0}")]
    DatabaseOpen(String),
    /// A named storage tree could not be opened.
    #[error("Tree open failed: {0}")]
    TreeOpen(String),
    /// A record could not be encoded for storage.
    #[error("Serialization error: {0}")]
    Serialization(String),
    /// Stored bytes could not be decoded as the expected record format.
    #[error("Deserialization error: {0}")]
    Deserialization(String),
    /// An insert or update transaction failed.
    #[error("Insert error: {0}")]
    Insert(String),
    /// Reading stored data failed.
    #[error("Query error: {0}")]
    Query(String),
    /// A deletion transaction failed.
    #[error("Delete error: {0}")]
    Delete(String),
}

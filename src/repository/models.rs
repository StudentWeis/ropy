//! Data model for clipboard records

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// Data model for clipboard records
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClipboardRecord {
    /// Unique identifier (timestamp in nanoseconds)
    pub id: u64,
    /// Clipboard content
    pub content: String,
    /// Creation time
    pub created_at: DateTime<Local>,
    /// Content type
    pub content_type: ContentType,
}

/// Content type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ContentType {
    /// Plain text
    Text,
    /// Image (stored as base64)
    Image,
    /// File path
    FilePath,
}

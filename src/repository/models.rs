//! Data model for clipboard records

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// Data model for clipboard records
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClipboardRecord {
    /// Unique identifier (content hash)
    pub id: u64,
    /// Clipboard content
    pub content: String,
    /// Creation time
    pub created_at: DateTime<Local>,
    /// Content type
    pub content_type: ContentType,
    /// Whether this record is pinned to the top
    #[serde(default)]
    pub pinned: bool,
}

/// Content type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentType {
    /// Plain text
    Text,
    /// Image (stored as `base64`)
    Image,
    /// File path
    FilePath,
}

impl ContentType {
    /// Encode content type as a single byte for the time index.
    pub(crate) const fn as_tag(&self) -> u8 {
        match self {
            Self::Text => 0,
            Self::Image => 1,
            Self::FilePath => 2,
        }
    }
}

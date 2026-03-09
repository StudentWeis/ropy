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
    /// Record category (default: `None`)
    #[serde(default)]
    pub category: Category,
}

/// Record category
///
/// A general-purpose classification for clipboard records.
/// `Pinned` records are always displayed at the top.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Category {
    /// Normal record without special treatment
    #[default]
    None,
    /// Pinned to the top of the list
    Pinned,
}

impl Category {
    /// Returns `true` if this category is `Pinned`.
    pub const fn is_pinned(&self) -> bool {
        matches!(self, Self::Pinned)
    }
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

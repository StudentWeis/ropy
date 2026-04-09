//! Data model for clipboard records

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// Data model for clipboard records
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClipboardRecord {
    /// Unique identifier (content hash)
    pub id: u64,
    /// Plain-text clipboard content used for display, search, and deduplication.
    pub content: String,
    /// Creation time
    pub created_at: DateTime<Local>,
    /// Content type
    pub content_type: ContentType,
    /// Whether this record is pinned to the top
    #[serde(default)]
    pub pinned: bool,
    /// Sidecar metadata for rich text payloads, when present.
    #[serde(default)]
    pub rich_text_meta: Option<RichTextMeta>,
}

/// Shared clipboard record list type alias for thread-safe access
pub type SharedRecords = Arc<RwLock<Vec<ClipboardRecord>>>;

/// Content type enumeration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentType {
    /// Plain text
    Text,
    /// Image (stored as `base64`)
    Image,
    /// File path
    FilePath,
    /// Rich text with plain-text summary plus optional HTML / RTF sidecars.
    RichText,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RichTextMeta {
    pub html_path: Option<String>,
    pub rtf_path: Option<String>,
}

impl ContentType {
    /// Encode content type as a single byte for the time index.
    pub(crate) const fn as_tag(&self) -> u8 {
        match self {
            Self::Text => 0,
            Self::Image => 1,
            Self::FilePath => 2,
            Self::RichText => 3,
        }
    }
}

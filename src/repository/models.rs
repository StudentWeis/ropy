//! Data model for clipboard records.

use std::sync::{Arc, RwLock};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClipboardRecord {
    /// Content hash; doubles as the deduplication key in the records tree.
    pub id: u64,
    /// Plain-text payload used for display, search, and dedup. Non-text
    /// payloads (image, files, rich text) keep their binary data on disk
    /// via [`RichTextMeta`] / sidecar files and store only a textual
    /// summary here.
    pub content: String,
    pub created_at: DateTime<Local>,
    pub content_type: ContentType,
    /// Pinned records stay at the top of the board and survive cleanup.
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub rich_text_meta: Option<RichTextMeta>,
}

pub type SharedRecords = Arc<RwLock<Vec<ClipboardRecord>>>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentType {
    Text,
    /// Image payload; the actual bytes live on disk under the per-record
    /// sidecar directory, and `content` carries the file path.
    Image,
    FilePath,
    /// Rich text with plain-text summary in `content` plus optional HTML / RTF
    /// sidecars referenced from [`RichTextMeta`].
    RichText,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RichTextMeta {
    pub html_path: Option<String>,
    pub rtf_path: Option<String>,
}

impl ContentType {
    /// One-byte tag used by the time index value format. Stable on disk —
    /// changing existing variants is a schema migration.
    pub(crate) const fn as_tag(&self) -> u8 {
        match self {
            Self::Text => 0,
            Self::Image => 1,
            Self::FilePath => 2,
            Self::RichText => 3,
        }
    }
}

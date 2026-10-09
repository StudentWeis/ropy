//! Data model for clipboard records.

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};

/// Persisted clipboard record; field order is part of the postcard storage format.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct ClipboardRecord {
    /// Content hash; doubles as the deduplication key in the records tree.
    pub id: u64,
    /// Plain-text payload used for display, search, and dedup. Non-text
    /// payloads (image, files, rich text) keep their binary data on disk
    /// via [`RichTextMeta`] / sidecar files and store only a textual
    /// summary here.
    pub content: String,
    /// Most recent capture time, including recaptures of identical content.
    pub created_at: DateTime<Local>,
    /// Interpretation of the content field and its sidecar payloads.
    pub content_type: ContentType,
    /// Pinned records stay at the top of the board and survive cleanup.
    #[serde(default)]
    pub pinned: bool,
    /// Optional paths for the HTML and RTF representations.
    #[serde(default)]
    pub rich_text_meta: Option<RichTextMeta>,
}

/// Stable content categories used by persisted records and content hashes.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentType {
    /// Plain text stored inline in the record.
    Text,
    /// Image payload; the actual bytes live on disk under the per-record
    /// sidecar directory, and `content` carries the file path.
    Image,
    /// A serialized list of filesystem paths.
    FilePath,
    /// Rich text with plain-text summary in `content` plus optional HTML / RTF
    /// sidecars referenced from [`RichTextMeta`].
    RichText,
}

/// Persisted locations of a rich-text record's optional representations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct RichTextMeta {
    /// Path to the HTML sidecar, if captured.
    pub html_path: Option<String>,
    /// Path to the RTF sidecar, if captured.
    pub rtf_path: Option<String>,
}

impl ContentType {
    /// One-byte tag used by the time index value format. Stable on disk —
    /// changing existing variants is a schema migration.
    #[must_use]
    pub const fn as_tag(&self) -> u8 {
        match self {
            Self::Text => 0,
            Self::Image => 1,
            Self::FilePath => 2,
            Self::RichText => 3,
        }
    }
}

impl ClipboardRecord {
    /// Construct an unpinned record without rich-text sidecars.
    #[must_use]
    pub const fn new(
        id: u64,
        content: String,
        created_at: DateTime<Local>,
        content_type: ContentType,
    ) -> Self {
        Self {
            id,
            content,
            created_at,
            content_type,
            pinned: false,
            rich_text_meta: None,
        }
    }

    /// Set whether this record stays above ordinary history.
    #[must_use]
    pub const fn pinned(mut self, pinned: bool) -> Self {
        self.pinned = pinned;
        self
    }

    /// Attach optional persisted HTML/RTF sidecar paths.
    #[must_use]
    pub fn with_rich_text_meta(mut self, meta: Option<RichTextMeta>) -> Self {
        self.rich_text_meta = meta;
        self
    }
}

impl RichTextMeta {
    /// Construct sidecar metadata from independently optional format paths.
    #[must_use]
    pub const fn new(html_path: Option<String>, rtf_path: Option<String>) -> Self {
        Self {
            html_path,
            rtf_path,
        }
    }
}

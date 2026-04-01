# Rich Text Support

## Background

Ropy currently captures three `ContentType` variants: `Text`, `Image`, and `FilePath`. When users copy content from applications like Word, web browsers, or rich text editors, the system clipboard simultaneously contains **multiple formats** (plain text + HTML + RTF), but Ropy only captures the plain text portion, **losing all formatting**.

The `clipboard-rs` crate (v0.3.3) already provides native support for `get_html()`, `get_rich_text()`, `set_html()`, `set_rich_text()`, and batch read/write via `get()`/`set()` APIs. The underlying capability is ready.

## Goals

1. **Lossless paste**: Rich text copied by users should retain original formatting (HTML/RTF) when pasted back.
2. **Backward compatible**: No breaking changes to existing text/image records; smooth database schema upgrade.
3. **Storage efficient**: Rich text metadata stored on-demand; pure text records incur no extra overhead.
4. **Display friendly**: GUI list shows plain text summary; a small icon indicates rich text availability.
5. **Minimal invasion**: Reuse existing architecture; changes concentrated at clear boundaries.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    System Clipboard                      │
│  ┌──────┐  ┌──────┐  ┌─────┐  ┌───────┐  ┌──────────┐  │
│  │ Text │  │ HTML │  │ RTF │  │ Image │  │ FilePath │  │
│  └──┬───┘  └──┬───┘  └──┬──┘  └───┬───┘  └────┬─────┘  │
└─────┼─────────┼─────────┼─────────┼────────────┼────────┘
      │         │         │         │            │
      ▼         ▼         ▼         ▼            ▼
┌─────────────────────────────────────────────────────────┐
│              ClipboardMonitor (listener.rs)              │
│  on_clipboard_change():                                  │
│    1. Image detection (existing, highest priority)        │
│    2. get_text() → plain_text                            │
│    3. has(Html) / has(Rtf) → detect richness             │
│    4. get_html() / get_rich_text() if available          │
│    5. Emit ClipboardEvent::RichText or ::Text            │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│              Repository (repo.rs)                        │
│  save_rich_text(plain, html, rtf):                       │
│    - ID = hash(plain_text)  (dedup by text content)      │
│    - Store ClipboardRecord with ContentType::RichText    │
│    - html/rtf → saved as files in rich_text/ directory   │
└──────────────────────┬──────────────────────────────────┘
                       │
                       ▼
┌─────────────────────────────────────────────────────────┐
│              GUI                                         │
│  List: show plain_text with rich text indicator icon     │
│  Paste: write back all original formats via set()        │
└─────────────────────────────────────────────────────────┘
```

## Detailed Design

### 1. Data Model (`src/repository/models.rs`)

Add `RichText` variant to `ContentType` and a new `RichTextMeta` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentType {
    Text,
    Image,
    FilePath,
    RichText, // new
}

impl ContentType {
    pub(crate) const fn as_tag(&self) -> u8 {
        match self {
            Self::Text => 0,
            Self::Image => 1,
            Self::FilePath => 2,
            Self::RichText => 3, // new
        }
    }
}

/// Rich text metadata referencing external files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RichTextMeta {
    /// HTML file name relative to the rich_text/ directory.
    pub html_path: Option<String>,
    /// RTF file name relative to the rich_text/ directory.
    pub rtf_path: Option<String>,
}
```

Add an optional field to `ClipboardRecord`:

```rust
pub struct ClipboardRecord {
    pub id: u64,
    /// Plain text content. Always populated for RichText records (used for search and display).
    pub content: String,
    pub created_at: DateTime<Local>,
    pub content_type: ContentType,
    #[serde(default)]
    pub pinned: bool,
    /// Rich text metadata. Only present for RichText records.
    #[serde(default)]
    pub rich_text_meta: Option<RichTextMeta>,
}
```

**Key decisions**:
- `content` always stores plain text → search, display, and dedup logic remain unchanged.
- HTML/RTF stored as files → avoids sled value bloat and serialization overhead.
- `#[serde(default)]` on `rich_text_meta` → old records deserialize with `None`, fully backward compatible.

### 2. File Storage Layout (`src/clipboard/utils.rs`)

```
~/.local/share/ropy/          (data_local_dir)
├── clipboard.db/             (sled database)
├── images/                   (existing)
│   ├── {id}.png
│   └── {id}_thumb.png
└── rich_text/                (new)
    ├── {id}.html
    └── {id}.rtf
```

New functions:

```rust
/// Save rich text content to files, returning RichTextMeta.
pub fn save_rich_text_files(
    record_id: u64,
    html: Option<&str>,
    rtf: Option<&str>,
) -> Option<RichTextMeta>;

/// Load HTML content from a rich text file.
pub fn load_rich_text_html(meta: &RichTextMeta) -> Option<String>;

/// Load RTF content from a rich text file.
pub fn load_rich_text_rtf(meta: &RichTextMeta) -> Option<String>;

/// Remove rich text files associated with a record.
pub fn remove_rich_text_files(meta: &RichTextMeta);
```

### 3. Clipboard Event (`src/clipboard/mod.rs`)

Add a new event variant:

```rust
pub enum ClipboardEvent {
    Text(String),
    Image(String, u64),
    /// Rich text: plain_text + optional HTML + optional RTF.
    RichText {
        plain_text: String,
        html: Option<String>,
        rtf: Option<String>,
    },
}
```

Add a corresponding `CopyRequest` variant:

```rust
pub enum CopyRequest {
    Text { text: String, completion: Option<CompletionSender<()>> },
    Image { path: String, completion: Option<CompletionSender<()>> },
    /// Write back all original formats simultaneously.
    RichText {
        plain_text: String,
        html: Option<String>,
        rtf: Option<String>,
        completion: Option<CompletionSender<()>>,
    },
}
```

### 4. Clipboard Listener (`src/clipboard/listener.rs`)

Modify `on_clipboard_change()` to detect and capture rich text formats:

```rust
fn on_clipboard_change(&mut self) {
    // ... existing lock acquisition ...

    // 1. Image detection (highest priority, unchanged)
    if let Ok(image) = self.ctx.get_image() /* ... */ {
        // ... existing image logic ...
        return;
    }

    // 2. Text detection
    if let Ok(text) = self.ctx.get_text()
        && !matches!(*last_copy_guard, LastCopyState::Text(ref last) if *last == text)
    {
        // 3. Detect rich text formats
        let has_html = self.ctx.has(ContentFormat::Html);
        let has_rtf = self.ctx.has(ContentFormat::Rtf);

        if has_html || has_rtf {
            let html = if has_html { self.ctx.get_html().ok() } else { None };
            let rtf = if has_rtf { self.ctx.get_rich_text().ok() } else { None };

            let _ = self.tx.send_blocking(ClipboardEvent::RichText {
                plain_text: text.clone(),
                html,
                rtf,
            });
        } else {
            let _ = self.tx.send_blocking(ClipboardEvent::Text(text.clone()));
        }

        *last_copy_guard = LastCopyState::Text(text);
    }
}
```

**Key points**:
- Dedup remains based on plain text content (`LastCopyState::Text`).
- Image priority is higher than rich text (preserves existing behavior).
- `has()` check is very lightweight, no performance impact.

### 5. Repository (`src/repository/repo.rs`)

Add a new save method:

```rust
/// Save a rich text record.
pub fn save_rich_text(
    &self,
    plain_text: String,
    html: Option<String>,
    rtf: Option<String>,
) -> Result<ClipboardRecord, RepositoryError> {
    let id = content_hash(&plain_text, &ContentType::RichText);
    let key = id.to_be_bytes();
    let now = Local::now();

    let rich_text_meta = save_rich_text_files(id, html.as_deref(), rtf.as_deref());

    if let Some(existing) = self.get_raw(&key)? {
        let mut record: ClipboardRecord = postcard::from_bytes(&existing)
            .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
        record.created_at = now;
        if rich_text_meta.is_some() {
            record.rich_text_meta = rich_text_meta;
        }
        self.put_raw(&key, &record)?;
        self.time_index.upsert(&record)?;
        return Ok(record);
    }

    let record = ClipboardRecord {
        id,
        content: plain_text,
        created_at: now,
        content_type: ContentType::RichText,
        pinned: false,
        rich_text_meta,
    };
    self.put_raw(&key, &record)?;
    self.time_index.upsert(&record)?;
    Ok(record)
}
```

### 6. Event Handler (`src/app.rs`)

Add a new match arm in `start_clipboard_event_handler`:

```rust
let result = match event {
    ClipboardEvent::Text(text) => repo.save_text(text),
    ClipboardEvent::Image(path, hash) => repo.save_image_from_path(path, hash),
    ClipboardEvent::RichText { plain_text, html, rtf } => {
        repo.save_rich_text(plain_text, html, rtf)
    }
};
```

### 7. Clipboard Writer (`src/clipboard/writer.rs`)

Handle the new `CopyRequest::RichText` variant using the batch `set()` API:

```rust
CopyRequest::RichText { plain_text, html, rtf, completion } => {
    let mut contents = vec![ClipboardContent::Text(plain_text)];
    if let Some(html_content) = html {
        contents.push(ClipboardContent::Html(html_content));
    }
    if let Some(rtf_content) = rtf {
        contents.push(ClipboardContent::Rtf(rtf_content));
    }
    let _ = ctx.set(contents);
    notify_completion(completion);
}
```

Using `ctx.set(contents)` writes all formats atomically, ensuring the target application sees all formats and achieves **lossless paste**.

### 8. GUI Changes

- **List view**: No layout changes. Add a small rich text indicator icon (e.g., `Rt` badge) on `RichText` records.
- **Preview panel**: Show plain text content (same as today). No HTML rendering needed.
- **Paste action**: When pasting a `RichText` record, send `CopyRequest::RichText` instead of `CopyRequest::Text`, loading HTML/RTF from files via `RichTextMeta`.

### 9. Cleanup Logic

Extend existing `delete` and `cleanup_old_records` methods:

```rust
// When deleting a record, also clean up associated rich text files
if record.content_type == ContentType::RichText {
    if let Some(ref meta) = record.rich_text_meta {
        remove_rich_text_files(meta);
    }
}
```

## Database Migration

- Bump `SCHEMA_VERSION` from 3 → 4.
- `rich_text_meta` uses `#[serde(default)]`, so old records deserialize with `None` automatically.
- Old records only contain `Text`/`Image`/`FilePath` variants, so no `RichText` deserialization issues.
- **No destructive migration needed**: old records remain fully compatible.

## Risks and Considerations

- **Storage growth**: HTML/RTF content can be large (tens of KB to MB). Consider adding a "store plain text only" option in settings.
- **`content_hash` consistency**: `ContentType::RichText` participates in hash calculation. If the same text is first stored as `Text` then as `RichText`, they produce different hashes. Consider using `ContentType::Text` for hash calculation of `RichText` records to enable cross-type dedup.
- **Clipboard race condition**: `get_html()` and `get_rich_text()` calls should immediately follow `get_text()` to minimize the window where another application could modify the clipboard.
- **Platform differences**: RTF support quality varies across platforms. HTML is more universally supported and should be prioritized.

## Implementation Phases

| Phase | Scope | Complexity |
|-------|-------|------------|
| **P0** | Data model + Listener + Storage + Writer | Medium |
| **P1** | GUI rich text indicator icon | Low |
| **P2** | Cleanup logic + Storage statistics | Low |

After **P0**, users can experience the full flow: copy from Word/browser → Ropy records → paste back with formatting preserved.

## Affected Modules

| Module | File(s) | Change Type |
|--------|---------|-------------|
| Models | `src/repository/models.rs` | Add `RichText` variant, `RichTextMeta` struct, field on `ClipboardRecord` |
| Utils | `src/clipboard/utils.rs` | Add `save_rich_text_files`, `load_rich_text_html`, `load_rich_text_rtf`, `remove_rich_text_files` |
| Events | `src/clipboard/mod.rs` | Add `ClipboardEvent::RichText`, `CopyRequest::RichText` |
| Listener | `src/clipboard/listener.rs` | Detect and capture HTML/RTF in `on_clipboard_change` |
| Repository | `src/repository/repo.rs` | Add `save_rich_text` method, extend cleanup |
| App | `src/app.rs` | Add `RichText` match arm in event handler |
| Writer | `src/clipboard/writer.rs` | Handle `CopyRequest::RichText` with batch `set()` |
| GUI | `src/gui/board/` | Add rich text icon, use `CopyRequest::RichText` for paste |

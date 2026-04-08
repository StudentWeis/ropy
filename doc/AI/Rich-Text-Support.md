# Rich Text Support

## Status

This document is a design proposal, not a description of the current implementation.

Current baseline in the repository as of 2026-04-02:

- `ContentType` supports `Text`, `Image`, and `FilePath`.
- `ClipboardEvent` supports `Text`, `Image`, and `Files`.
- `CopyRequest` supports `Text`, `Image`, and `Files`.
- Clipboard listener priority is `Image -> Files -> Text`.
- The repository defaults to `redb` (`clipboard.redb`) and also supports `sled` (`clipboard.db`).

Rich text capture, storage, and lossless paste-back are not implemented yet.

## Background

Ropy currently preserves three clipboard content categories: plain text, images, and file lists. When users copy content from applications like Word, web browsers, or rich text editors, the system clipboard often contains multiple formats at once, such as plain text, HTML, and RTF. Ropy currently reads the plain text portion only, so formatting is lost when the item is copied back out of Ropy.

The `clipboard-rs` crate (v0.3.3) already exposes `get_html()`, `get_rich_text()`, `set_html()`, `set_rich_text()`, and batch `set()` APIs, so the lower-level clipboard capability is available.

## Goals

1. **Lossless paste**: rich text copied by users should retain original formatting when pasted back.
2. **Fit the current architecture**: keep the change localized to the clipboard, repository, and board copy path.
3. **Storage efficient**: rich text payloads should be stored only when present; plain text records should not pay extra cost.
4. **Display friendly**: the list should still render a plain text summary, with a lightweight indicator that rich text is available.
5. **Preserve existing behavior**: image and file handling should continue to work exactly as they do today.

## Architecture Overview

```text
System Clipboard
  Text / HTML / RTF / Image / FilePath
        |
        v
ClipboardMonitor (listener.rs)
  1. Image detection (existing, highest priority)
  2. File-list detection (existing)
  3. Plain-text read (existing)
  4. If text exists, probe HTML / RTF and emit RichText when available
        |
        v
Repository (repo.rs)
  save_text / save_files / save_image_from_path / save_rich_text
  rich text payload stored as sidecar files under rich_text/
        |
        v
GUI
  list renders plain-text summary
  rich-text records show an indicator badge
  paste path uses CopyRequest::RichText to restore all formats
```

## Detailed Design

### 1. Data Model (`src/repository/models.rs`)

Add `RichText` to `ContentType` and a `RichTextMeta` sidecar descriptor:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentType {
    Text,
    Image,
    FilePath,
    RichText,
}

impl ContentType {
    pub(crate) const fn as_tag(&self) -> u8 {
        match self {
            Self::Text => 0,
            Self::Image => 1,
            Self::FilePath => 2,
            Self::RichText => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RichTextMeta {
    pub html_path: Option<String>,
    pub rtf_path: Option<String>,
}
```

Extend `ClipboardRecord` with optional metadata:

```rust
pub struct ClipboardRecord {
    pub id: u64,
    /// Plain-text content, used for display, search, and dedup.
    pub content: String,
    pub created_at: DateTime<Local>,
    pub content_type: ContentType,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default)]
    pub rich_text_meta: Option<RichTextMeta>,
}
```

Key decisions:

- `content` remains the plain-text summary even for rich text records.
- HTML and RTF are stored outside the main KV value to avoid inflating record size.
- `rich_text_meta` uses `#[serde(default)]` so older records can deserialize without the field.

### 2. File Storage Layout (`src/clipboard/utils.rs`)

```text
~/.local/share/ropy/
├── clipboard.redb    # default redb backend
├── clipboard.db      # optional sled backend
├── images/           # existing
│   ├── {id}.png
│   └── {id}_thumb.png
└── rich_text/        # new
    ├── {id}.html
    └── {id}.rtf
```

Add helper functions:

```rust
pub fn save_rich_text_files(
    record_id: u64,
    html: Option<&str>,
    rtf: Option<&str>,
) -> Option<RichTextMeta>;

pub fn load_rich_text_html(meta: &RichTextMeta) -> Option<String>;

pub fn load_rich_text_rtf(meta: &RichTextMeta) -> Option<String>;

pub fn remove_rich_text_files(meta: &RichTextMeta);
```

### 3. Clipboard Event (`src/clipboard/mod.rs`)

Keep existing text, image, and file-list events, then add rich text:

```rust
pub enum ClipboardEvent {
    Text(String),
    /// Image(path, `content_hash`)
    Image(String, u64),
    Files(Vec<String>),
    RichText {
        plain_text: String,
        html: Option<String>,
        rtf: Option<String>,
    },
}
```

Do the same for clipboard writes:

```rust
pub enum CopyRequest {
    Text {
        text: String,
        completion: Option<CompletionSender<()>>,
    },
    Image {
        path: String,
        completion: Option<CompletionSender<()>>,
    },
    Files {
        paths: Vec<String>,
        completion: Option<CompletionSender<()>>,
    },
    RichText {
        plain_text: String,
        html: Option<String>,
        rtf: Option<String>,
        completion: Option<CompletionSender<()>>,
    },
}
```

### 4. Clipboard Listener (`src/clipboard/listener.rs`)

The listener should keep the current detection order and extend only the text branch:

```rust
fn on_clipboard_change(&mut self) {
    let mut last_copy_guard = lock_or_recover(&self.last_copy);

    if let Ok(image) = self.ctx.get_image()
        && let Ok(dyn_img) = image.get_dynamic_image()
    {
        // existing image path
        return;
    }

    let files = self
        .ctx
        .get_files()
        .ok()
        .map(|paths| normalize_file_paths(&paths));

    if let Some(files) = files.filter(|paths| !paths.is_empty()) {
        // existing file-list path
        return;
    }

    if let Ok(text) = self.ctx.get_text()
        && should_forward_text(&last_copy_guard, &text)
    {
        let has_html = self.ctx.has(ContentFormat::Html);
        let has_rtf = self.ctx.has(ContentFormat::Rtf);

        if has_html || has_rtf {
            let html = has_html.then(|| self.ctx.get_html().ok()).flatten();
            let rtf = has_rtf.then(|| self.ctx.get_rich_text().ok()).flatten();

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

Key points:

- Image priority remains highest.
- File-list priority remains above text so copied files are not downgraded to plain text.
- Rich text dedup can continue to reuse `LastCopyState::Text` based on plain text.

### 5. Repository (`src/repository/repo.rs`)

Add a save method alongside the existing `save_text` and `save_files` helpers:

```rust
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

Note: this keeps rich text dedup consistent with the existing `content_hash(content, content_type)` approach. If cross-type dedup with plain text is desired, that should be a separate explicit decision.

### 6. Event Handler (`src/app.rs`)

Extend the current match instead of replacing it:

```rust
let result = match event {
    ClipboardEvent::Text(text) => repo.save_text(text),
    ClipboardEvent::Image(path, hash) => repo.save_image_from_path(path, hash),
    ClipboardEvent::Files(paths) => repo.save_files(&paths),
    ClipboardEvent::RichText {
        plain_text,
        html,
        rtf,
    } => repo.save_rich_text(plain_text, html, rtf),
};
```

### 7. Clipboard Writer (`src/clipboard/writer.rs`)

Add a `RichText` write path using the same batch `set()` style already used for file payloads:

```rust
CopyRequest::RichText {
    plain_text,
    html,
    rtf,
    completion,
} => {
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

### 8. GUI Changes

- **List view**: keep the current layout and render the plain-text summary; add a small rich text badge or icon to `RichText` rows.
- **Preview panel**: continue to show the plain-text summary; HTML rendering is not required for the first iteration.
- **Paste action**: when the selected record is `RichText`, load HTML and RTF from `RichTextMeta` and send `CopyRequest::RichText`.
- **Filters**: no dedicated rich text filter is required for the first iteration.

### 9. Cleanup Logic

Extend the existing image cleanup behavior to cover rich text sidecar files too:

```rust
if record.content_type == ContentType::Image {
    Self::remove_image_files(&record.content);
}

if let Some(meta) = record.rich_text_meta.as_ref() {
    remove_rich_text_files(meta);
}
```

This applies to:

- `delete`
- `cleanup_old_records`
- `clear`
- schema-migration cleanup, if a destructive migration path is ever used

## Persistence and Migration

- The current repository `SCHEMA_VERSION` is already `4`.
- The current schema-migration path is destructive: if the stored schema version differs, the repository clears records, time index entries, favorites, and persisted images.
- Because this proposal adds only a new enum tag and an optional `rich_text_meta` field with `#[serde(default)]`, it should be possible to keep the schema version unchanged as long as the on-disk key/value encoding does not change.
- If implementation later requires a schema bump, document that the current migration path will wipe stored history unless a non-destructive migration flow is added first.

## Risks and Considerations

- **Storage growth**: HTML and RTF content can be much larger than plain text. Consider a future setting to store plain text only.
- **Cross-type dedup**: `content_hash` currently includes the content type tag. A plain text item and a rich text item with the same visible text will be stored as different records.
- **Clipboard race window**: `get_html()` and `get_rich_text()` should happen immediately after `get_text()` to minimize drift if another app mutates the clipboard.
- **Platform differences**: HTML and RTF support varies by platform and application. HTML should be treated as the more portable format.
- **Cleanup parity**: any place that currently removes image sidecar files must also remove rich text sidecar files.

## Implementation Phases

| Phase | Scope | Complexity |
|-------|-------|------------|
| **P0** | Data model, listener, repository, writer | Medium |
| **P1** | Board copy path and list indicator | Low |
| **P2** | Cleanup hardening and optional storage controls | Low |

After **P1**, users should be able to copy from Word or a browser, store the item in Ropy, and paste it back with formatting preserved.

## Affected Modules

| Module | File(s) | Change Type |
|--------|---------|-------------|
| Models | `src/repository/models.rs` | Add `RichText` and `RichTextMeta` |
| Clipboard Utils | `src/clipboard/utils.rs` | Add rich text sidecar save/load/remove helpers |
| Events | `src/clipboard/mod.rs` | Add `ClipboardEvent::RichText` and `CopyRequest::RichText` without removing `Files` |
| Listener | `src/clipboard/listener.rs` | Probe HTML and RTF only after the existing image and file branches |
| Repository | `src/repository/repo.rs` | Add `save_rich_text` and rich text cleanup |
| App | `src/app.rs` | Add `RichText` handling to the clipboard event pipeline |
| Writer | `src/clipboard/writer.rs` | Write text plus optional HTML and RTF with batch `set()` |
| GUI | `src/gui/board/mod.rs`, `src/gui/board/records_list.rs` | Add paste support and a lightweight rich text indicator |

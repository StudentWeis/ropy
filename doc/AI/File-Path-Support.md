# File Path Support

## Background

Ropy already defines `ContentType::FilePath` in the repository model, and the repository can persist records with that type. However, the end-to-end clipboard pipeline is still only implemented for text and images.

Current gaps:

- The clipboard listener only forwards text and image payloads.
- The clipboard writer only writes text and image payloads back to the system clipboard.
- The application event handler only persists text and image clipboard events.
- The board confirm flow still panics for `ContentType::FilePath`.
- Search and record rendering treat file-path records as placeholders rather than first-class content.

This document records a pragmatic design for full file-path clipboard support without introducing unnecessary schema churn.

## Goals

1. Support copying files from the system clipboard into Ropy history.
2. Support restoring file selections back to the system clipboard when a record is confirmed.
3. Preserve compatibility with the existing repository schema and stored records.
4. Keep platform-specific clipboard behavior behind `clipboard-rs` instead of introducing native per-OS code in Ropy.
5. Provide a clear path for incremental implementation with tests.

## Non-Goals

1. Renaming `ContentType::FilePath` in the first iteration.
2. Adding drag-and-drop behavior.
3. Building a full file preview experience.
4. Introducing a schema migration unless the implementation later proves that one is necessary.

## Key Finding

The existing `clipboard-rs 0.3.3` dependency already provides first-class file clipboard support:

- `Clipboard::get_files() -> Result<Vec<String>>`
- `Clipboard::set_files(Vec<String>) -> Result<()>`
- `ClipboardContent::Files(Vec<String>)`
- `ContentFormat::Files`

The macOS, Windows, and X11 implementations are already present in the crate. That means Ropy does not need to add native clipboard glue for this feature. The work is primarily about wiring the existing capability into Ropy's listener, writer, repository flow, and UI.

## Design Principle

Treat file clipboard content as a file list, not as a single path string.

Although the current model name is `FilePath`, system clipboard semantics are list-based:

- Finder can copy multiple files.
- Explorer can copy multiple files.
- X11 file clipboard payloads are represented as `text/uri-list`.

So the implementation should model runtime clipboard behavior as `Vec<String>`, even if persistence temporarily keeps using `ContentType::FilePath`.

## Recommended Scope

Implement file support in two layers.

### Phase 1: Minimal-Invasive Support

Use the existing `ContentType::FilePath` variant and keep the current repository schema intact.

- Runtime clipboard events carry `Vec<String>`.
- Clipboard write requests carry `Vec<String>`.
- Persist file lists inside `ClipboardRecord.content`.
- Decode old single-path records and new file-list records through a compatibility parser.

This phase is the recommended starting point because it closes the functionality gap without forcing a migration.

### Phase 2: Optional Data Model Cleanup

If later desired, rename `ContentType::FilePath` to something like `Files` and formalize a dedicated persisted payload type.

This phase should only happen after Phase 1 is stable and validated in the field.

## Runtime Data Model

Add file-aware variants to the clipboard pipeline.

### Clipboard events

```rust
pub enum ClipboardEvent {
    Text(String),
    Image(String, u64),
    Files(Vec<String>),
}
```

### Clipboard write requests

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
}
```

### Last copy state

```rust
pub enum LastCopyState {
    Text(String),
    Image(u64),
    Files(u64),
}
```

For files, deduplication should use a deterministic hash over the normalized list.

## Persistence Format

### Why not store a raw newline-separated string?

Newline-separated text is ambiguous:

- file names can contain newlines,
- Linux clipboard formats may already use URI lists,
- compatibility parsing becomes fragile.

### Recommended persisted encoding

Store file-list content as JSON inside `ClipboardRecord.content`.

Examples:

- Single file: `["/Users/me/Desktop/demo.txt"]`
- Multiple files: `["/tmp/a.txt","/tmp/b.txt"]`

### Compatibility rule

When reading a `ContentType::FilePath` record:

1. Try to parse `record.content` as `Vec<String>` JSON.
2. If parsing fails, treat it as a legacy single path and return `vec![record.content.clone()]`.

This avoids a schema migration and keeps old records usable.

## Path Normalization

All file clipboard paths should go through a small normalization layer before hashing, comparing, storing, or writing.

Responsibilities:

1. Accept either local paths or `file://` URIs.
2. Convert `file://` values to local filesystem paths when possible.
3. Preserve the original order of the list.
4. Drop empty entries.
5. Avoid `canonicalize()` so missing files or permission differences do not break the pipeline.

Suggested helpers:

```rust
fn normalize_file_paths(paths: &[String]) -> Vec<String>;
fn serialize_file_payload(paths: &[String]) -> Result<String, serde_json::Error>;
fn deserialize_file_payload(content: &str) -> Vec<String>;
fn hash_file_paths(paths: &[String]) -> u64;
```

## Clipboard Listener Flow

The listener should change from a two-branch pipeline to a three-branch pipeline.

Recommended priority:

1. Image
2. Files
3. Text

Rationale:

- Images should keep the current highest priority because screenshots and copied images commonly expose image data alongside other formats.
- File selections should be captured before plain text, because many apps expose both file-list and textual path representations.
- Text remains the fallback.

### Listener behavior

1. Try `ctx.get_image()` as today.
2. If no image is available, try `ctx.get_files()`.
3. Normalize the list.
4. If the normalized list is non-empty and differs from `LastCopyState`, emit `ClipboardEvent::Files(paths)`.
5. If no files are available, fall back to `ctx.get_text()`.

## Clipboard Writer Flow

Add file writing to the shared clipboard writer task.

### Preferred behavior

For file records, write both:

- `ClipboardContent::Files(paths.clone())`
- `ClipboardContent::Text(paths.join("\n"))`

using the batch `set(...)` API.

Reasoning:

- File managers receive the file-list payload.
- Plain text fields still get a usable textual representation.
- This mirrors the crate's multi-format design better than only writing a single format.

### Fallback behavior

If batch `set(...)` proves unreliable on a target platform, fall back to `set_files(paths)`.

## Repository Flow

The repository already supports saving arbitrary content with `ContentType::FilePath`, so no structural repository rewrite is required in Phase 1.

Recommended addition:

```rust
pub fn save_files(&self, paths: Vec<String>) -> Result<ClipboardRecord, RepositoryError>;
```

Behavior:

1. Normalize the list.
2. Serialize it to JSON.
3. Save it using `ContentType::FilePath`.

This keeps file-specific persistence logic out of the UI and app orchestration layers.

## App Event Handling

Extend the clipboard event handler to persist files through the repository.

```rust
match event {
    ClipboardEvent::Text(text) => repo.save_text(text),
    ClipboardEvent::Image(path, hash) => repo.save_image_from_path(path, hash),
    ClipboardEvent::Files(paths) => repo.save_files(paths),
}
```

## Board Confirm Flow

The current `todo!()` for `ContentType::FilePath` should be replaced with file-aware clipboard writing.

Behavior:

1. Decode the stored payload into `Vec<String>`.
2. Create `CopyRequest::Files`.
3. Send it through the existing writer channel.
4. Respect the existing clipboard completion timeout logic.

This removes the current panic path and aligns file records with the existing confirm semantics for text and images.

## Search and Filtering

The current search logic only matches text records. File records should become searchable.

### Matching behavior

For `ContentType::FilePath` records, a query should match when any file path matches:

- full path,
- file name,
- directory segment.

The same case-sensitive and whole-word options can still be reused.

### Filter behavior

Recommended UI change:

- Add a dedicated `Files` content filter alongside `Text`, `Image`, and `Favorites`.

If a smaller first step is preferred, file records can still appear under `All` and `Favorites` before a separate filter button is added.

## Record Rendering

The list should stop rendering file records as a generic `File` placeholder.

Recommended display:

- Single file: show the base file name on the first line and the parent directory as metadata.
- Multiple files: show `N files` on the first line and the first one or two names in the preview.
- Tooltip or preview: show the full normalized list.

This keeps the list compact without hiding the actual content.

## Error Handling

The current clipboard I/O path silently discards some errors. File support is a good point to tighten that behavior.

At minimum:

- log failures from `get_files`, `set_files`, and `set` when they are expected to succeed,
- log channel send failures instead of discarding them,
- fail gracefully in the confirm path instead of panicking.

User-visible error affordances can be added later, but silent failure should not remain the default.

## Testing Strategy

Follow TDD and split tests by layer.

### Unit tests

1. `normalize_file_paths` converts `file://` values correctly.
2. `deserialize_file_payload` supports both JSON arrays and legacy single strings.
3. `hash_file_paths` is stable for equivalent normalized inputs.
4. `CopyRequest::files` constructors populate fields correctly.
5. listener dedup logic rejects identical consecutive file lists.

### Repository tests

1. `save_files` stores `ContentType::FilePath`.
2. duplicate file lists upsert rather than duplicating records.
3. legacy single-path records still decode correctly.

### Search tests

1. file records match by file name.
2. file records match by full path.
3. file records participate correctly in `Favorites` and `All` filters.
4. a dedicated `Files` filter returns only file records if implemented.

### UI and confirm-flow tests

1. confirming a file record no longer panics.
2. confirming a file record sends `CopyRequest::Files`.
3. record preview text is meaningful for single-file and multi-file records.

## Incremental Implementation Plan

### Step 1

Add shared helpers and tests for normalization, serialization, deserialization, and hashing.

### Step 2

Extend `ClipboardEvent`, `CopyRequest`, and `LastCopyState` with file-list support.

### Step 3

Update the listener to detect files before text.

### Step 4

Update the writer to write file lists back to the clipboard.

### Step 5

Add `save_files` to the repository and wire the app event handler.

### Step 6

Replace the board confirm-path `todo!()` and add tests.

### Step 7

Improve search, filters, and record rendering for file records.

## Tradeoffs

### Pros

1. Delivers file clipboard support without schema migration.
2. Reuses the existing `clipboard-rs` cross-platform abstraction.
3. Keeps platform complexity out of Ropy.
4. Preserves backward compatibility for stored records.

### Cons

1. `ContentType::FilePath` remains a slightly misleading name when multi-file payloads are stored.
2. JSON-in-string payloads are less explicit than a dedicated structured field.
3. Search and rendering logic need compatibility parsing until the model is cleaned up.

These tradeoffs are acceptable for the first implementation because they keep the risk surface small while removing a real functional gap.

## Recommendation

Adopt Phase 1 now.

It is the highest-leverage path:

- small surface area,
- no migration cost,
- no native platform work,
- immediately removes the current confirm-flow panic,
- leaves room for a later model cleanup if the feature proves valuable.

Once the feature is working end-to-end, reassess whether `ContentType::FilePath` should be renamed or whether a dedicated persisted payload type is worth the extra complexity.

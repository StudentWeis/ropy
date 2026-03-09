# Clipboard Record Deduplication Strategy

## Current Analysis

### Existing Deduplication Mechanism

Currently the system only relies on `LastCopyState` (in memory) for consecutive deduplication:

- Text: if the copied text equals the previous copy, it is skipped.
- Image: if the copied image has the same hash as the previous copy, it is skipped.

Limitations:

- Cannot prevent A → B → A from producing two separate A records.
- Conflicts with features like pinning and expiration cleanup, which increases code complexity and makes maintenance harder.

### Current Data Model

- `id`: a nanosecond timestamp, also used as the sled key.
- `get_recent()`: uses sled keys' lexicographic order reversed (since timestamps increase). `iter().rev().take(limit)` returns the latest N records.
- `cleanup_old_records()`: deletes the first N items from `iter()` (the oldest ones).

## New Proposal

### Core Idea

1. Change the meaning of `id`: from a nanosecond timestamp to a content hash. Keep using `id` as the primary key and as sled key. Identical content always maps to the same key.
2. Deduplicate on save: records with the same hash only update `created_at`; keep one unique record per content and update its timestamp.
3. Change sorting logic: `get_recent()` no longer relies on key order — load all records and sort by `created_at` descending.

### Modules to Modify

#### 1. `ClipboardRecord` (models.rs)

```rust
pub struct ClipboardRecord {
    /// Unique identifier (content hash)
    pub id: u64,
    pub content: String,
    pub created_at: DateTime<Local>,
    pub content_type: ContentType,
}
```

Update the `id` comment from "timestamp in nanoseconds" to "content hash". Keep the struct fields unchanged and reuse the `id` field, only changing its value source from timestamp to hash.

#### 2. Hash Function Choice

`DefaultHasher` is not stable across Rust versions. Because we persist hash values as sled keys, we must use a deterministic hash function.

Recommended: use the `seahash` crate (pure Rust, fast, deterministic):

```rust
fn content_hash(content: &str, content_type: &ContentType) -> u64 {
    use seahash::hash;
    // encode the content_type into the hash to avoid collisions across types
    let type_tag: u8 = match content_type {
        ContentType::Text => 0,
        ContentType::Image => 1,
        ContentType::FilePath => 2,
    };
    let mut data = vec![type_tag];
    data.extend_from_slice(content.as_bytes());
    hash(&data)
}
```

A u64 hash is sufficient for a clipboard manager with fewer than a few million records; collision probability is negligible.

#### 3. `save()` method (repo.rs)

Use the content hash as `id`, then upsert: if an identical hash exists, update `created_at`; otherwise insert a new record.

```rust
pub fn save(
    &self,
    content: String,
    content_type: ContentType,
) -> Result<ClipboardRecord, RepositoryError> {
    let id = content_hash(&content, &content_type);
    let key = id.to_be_bytes();
    let now = Local::now();

    // check if a record with the same hash exists
    if let Some(existing) = self.records_tree.get(&key)
        .map_err(|e| RepositoryError::Query(e.to_string()))?
    {
        // exists: only update created_at
        let mut record: ClipboardRecord = serde_json::from_slice(&existing)
            .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
        record.created_at = now;
        let value = serde_json::to_vec(&record)
            .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
        self.records_tree.insert(key, value)
            .map_err(|e| RepositoryError::Insert(e.to_string()))?;
        return Ok(record);
    }

    // not exist: create new record
    let record = ClipboardRecord {
        id,
        content,
        created_at: now,
        content_type,
    };
    let value = serde_json::to_vec(&record)
        .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
    self.records_tree.insert(key, value)
        .map_err(|e| RepositoryError::Insert(e.to_string()))?;

    Ok(record)
}
```

#### 4. Image Deduplication (by image content hash)

`content` currently stores the file path for images; copying the same image twice can produce different paths. Therefore image hash must be computed from the image bytes rather than the path.

The clipboard listener already computes a hash of image bytes for `LastCopyState`; propagate that hash through the listener chain into the repository.

##### 4.1 Update `ClipboardEvent` (clipboard/mod.rs)

```rust
pub enum ClipboardEvent {
    Text(String),
    /// Image(path, content_hash)
    Image(String, u64),
}
```

##### 4.2 Update `on_clipboard_change` (listener.rs)

Compute the image hash with `seahash` (deterministic) and send it through the channel with the image:

```rust
// send (dyn_img, hash)
let _ = self.image_tx.send_blocking((dyn_img, hash));
```

Change `image_tx` type from `Sender<DynamicImage>` to `Sender<(DynamicImage, u64)>`.

In `start_clipboard_monitor`:

```rust
while let Ok((image, hash)) = image_rx.recv().await {
    if let Some(path) = super::save_image(&image) {
        let _ = tx.send_blocking(ClipboardEvent::Image(path, hash));
    }
}
```

##### 4.3 `save_image_from_path()` (repo.rs)

Accept an externally computed content hash as `id` and handle duplicate image files by removing newly generated duplicates while keeping the old file.

```rust
pub fn save_image_from_path(
    &self,
    file_path: String,
    content_hash: u64,
) -> Result<ClipboardRecord, RepositoryError> {
    let id = content_hash;
    let key = id.to_be_bytes();
    let now = Local::now();

    // check if an image record with the same hash exists
    if let Some(existing) = self.records_tree.get(&key)
        .map_err(|e| RepositoryError::Query(e.to_string()))?
    {
        let mut record: ClipboardRecord = serde_json::from_slice(&existing)
            .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;

        // if path differs, delete the newly generated duplicate file (keep the old one)
        if record.content != file_path {
            let _ = fs::remove_file(&file_path);
            let thumb_path = file_path.replace(".png", "_thumb.png");
            let _ = fs::remove_file(thumb_path);
        }

        // only update created_at
        record.created_at = now;
        let value = serde_json::to_vec(&record)
            .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
        self.records_tree.insert(key, value)
            .map_err(|e| RepositoryError::Insert(e.to_string()))?;
        return Ok(record);
    }

    // not exist: create new record
    let record = ClipboardRecord {
        id,
        content: file_path,
        created_at: now,
        content_type: ContentType::Image,
    };
    let value = serde_json::to_vec(&record)
        .map_err(|e| RepositoryError::Serialization(e.to_string()))?;
    self.records_tree.insert(key, value)
        .map_err(|e| RepositoryError::Insert(e.to_string()))?;

    Ok(record)
}
```

##### 4.4 Update `start_clipboard_listener` (listener.rs)

Handle the new `ClipboardEvent::Image` variant:

```rust
let result = match event {
    ClipboardEvent::Text(text) => repo.save_text(text),
    ClipboardEvent::Image(path, hash) => repo.save_image_from_path(path, hash),
};
```

#### 5. `get_recent()` (repo.rs)

Stop relying on key ordering. Load all records and sort by `created_at` descending:

```rust
pub fn get_recent(&self, limit: usize) -> Result<Vec<ClipboardRecord>, RepositoryError> {
    let mut records = Vec::new();
    for result in self.records_tree.iter() {
        let (_, value) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
        let record: ClipboardRecord = serde_json::from_slice(&value)
            .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
        records.push(record);
    }
    records.sort_unstable_by(|a, b| b.created_at.cmp(&a.created_at));
    records.truncate(limit);
    Ok(records)
}
```

Performance: full load + sort is O(n log n). For a clipboard manager (typically < 1000 records), this is acceptable.

This means `get_recent()`, `search()`, and `cleanup_old_records()` should all use `created_at` for ordering rather than sled key order.

#### 6. `cleanup_old_records()` (repo.rs)

Also switch to loading all records and deleting the oldest by `created_at`:

```rust
pub fn cleanup_old_records(&self, keep_count: usize) -> Result<usize, RepositoryError> {
    let total = self.count();
    if total <= keep_count {
        return Ok(0);
    }

    let mut records: Vec<(u64, DateTime<Local>)> = Vec::new();
    for result in self.records_tree.iter() {
        let (_, value) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
        let record: ClipboardRecord = serde_json::from_slice(&value)
            .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;
        records.push((record.id, record.created_at));
    }
    // sort ascending by created_at; earliest entries first
    records.sort_unstable_by(|a, b| a.1.cmp(&b.1));

    let to_remove = total - keep_count;
    let mut removed = 0;
    for (id, _) in records.into_iter().take(to_remove) {
        let key = id.to_be_bytes();
        self.records_tree
            .remove(key)
            .map_err(|e| RepositoryError::Delete(e.to_string()))?;
        removed += 1;
    }

    Ok(removed)
}
```

#### 7. `search()` (repo.rs)

The current implementation already iterates all records, but ensure results are returned ordered by `created_at` descending:

```rust
pub fn search(&self, keyword: &str) -> Result<Vec<ClipboardRecord>, RepositoryError> {
    let keyword_lower = keyword.to_lowercase();
    let mut records = Vec::new();

    for result in self.records_tree.iter() {
        let (_, value) = result.map_err(|e| RepositoryError::Query(e.to_string()))?;
        let record: ClipboardRecord = serde_json::from_slice(&value)
            .map_err(|e| RepositoryError::Deserialization(e.to_string()))?;

        if record.content_type == ContentType::Text
            && record.content.to_lowercase().contains(&keyword_lower)
        {
            records.push(record);
        }
    }

    records.sort_unstable_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(records)
}
```

#### 8. In-memory `shared_records` maintenance (listener.rs)

When `save()` returns a record (new or updated), the in-memory `shared_records` should remove any existing record with the same id first, then insert the returned record at the front:

```rust
let mut guard = match shared_records.lock() { ... };
// remove any record with the same id (dedupe/update scenario)
guard.retain(|r| r.id != record.id);
guard.insert(0, record);
```

#### 9. UI behavior on selecting a record (board/mod.rs)

Currently the UI, when selecting a non-first record, copies the content to clipboard and then deletes the old record. Under the new global dedupe model, that deletion is no longer necessary.

`confirm_record()` should instead:

1. Write the record content back to the clipboard.
2. Close the panel (unless pinned).
3. Do not proactively delete the record.

The clipboard listener will capture this copy event; repository upsert semantics (id = content hash) will:

- update `created_at` and move the record to the top if it exists, or
- insert a new record if it does not exist.

This centralizes deduplication and "move-to-top" behavior in the repository layer and avoids UI-side deletions.

### Data Migration

Changing the meaning of `id` means old database keys (timestamps) cannot be interpreted as new hash keys. Migration options:

1. Full migration: detect old-format data (e.g., a `schema_version` in `meta` tree), iterate old records, reinsert them with new hash keys, and delete old keys, then write new version.
2. Simpler: clear old data after bumping schema version, since clipboard history is not critical.

Recommendation: bump `SCHEMA_VERSION = 2` and clear the `clipboard_records` tree and image directory when the stored `schema_version` does not match, avoiding complex online migration.

### Change Summary

| Module | Changes |
|--------|---------|
| `models.rs` | update `id` comment |
| `clipboard/mod.rs` | `ClipboardEvent::Image` carries `u64` hash |
| `repo.rs` | add `content_hash()` function |
| `repo.rs` | `save()` upserts using hash as id |
| `repo.rs` | `save_image_from_path()` accepts hash and de-duplicates files |
| `repo.rs` | `get_recent()` sorts by `created_at` |
| `repo.rs` | `cleanup_old_records()` sorts by `created_at` |
| `repo.rs` | `search()` sorts by `created_at` |
| `listener.rs` | compute image hash with `seahash` and pass it through channels |
| `listener.rs` | remove old record with same id before inserting into `shared_records` |
| `board/mod.rs` | selecting a record no longer deletes it proactively |
| `Cargo.toml` | add `seahash` dependency |

## Implementation Status

**Completed** — 2026-03-09

All changes have been implemented as described above and passed unit tests (including three new deduplication tests) and clippy checks.

Data migration uses the "bump schema version and clear old DB" approach: at startup check `meta` tree for `schema_version`; if it differs from `SCHEMA_VERSION = 2`, clear the `clipboard_records` tree and image directory.

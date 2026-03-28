# Clipboard Record Storage Architecture

## Overview

Ropy uses a two-layer storage model:

- a persistent `sled` repository for long-term retention,
- a smaller in-memory cache that backs the visible board.

This split keeps the UI responsive while avoiding full-database scans during normal interaction.

## Persistent Storage Layer

The repository currently uses three `sled` trees:

- `clipboard_records`: postcard-serialized `ClipboardRecord` values.
- `time_index`: compact chronological index used for recent-item selection and cleanup.
- `favorites`: favorite membership and favorite ordering metadata.

### Record identity and deduplication

- Text records use a content-derived hash from `content_hash(content, ContentType)`.
- Image records use the image byte hash produced by the clipboard listener.
- Saving duplicate content updates `created_at` on the existing record instead of creating a second row.
- Pin state is preserved when a record is updated in place.

### Schema management

- Current schema version: `3`.
- On schema mismatch, the repository clears records, time index, favorites, and persisted image files before writing the new version marker.

## In-Memory Cache Layer

The board keeps recent records in `Arc<Mutex<Vec<ClipboardRecord>>>`.

Important scope detail:

- This cache is intentionally limited to `max_history_records`.
- It is not a mirror of the full repository.
- Search and filtering operate on this cache, not on all stored records.

The board also keeps separate in-memory state for:

- `filtered_record_indices` for the active search/filter result set,
- `favorite_ids` for fast favorites filtering,
- search options and content filters.

## Synchronization Flow

### Add or upsert a record

1. Clipboard event arrives from the listener.
2. Repository saves or upserts the record.
3. Foreground task removes any cached duplicate by ID.
4. The new record is inserted at the front of the in-memory cache.
5. If the cache exceeds `max_history_records`, it is truncated.
6. Repository cleanup runs against `max_storage_records`.

### Delete a record

1. Repository deletes the record from `clipboard_records`.
2. Related favorite membership is removed.
3. Related image files are deleted for image records.
4. The in-memory cache removes the record by ID.

### Toggle pin

1. Repository flips `record.pinned` and updates `time_index`.
2. The in-memory cache updates the same record in place.

### Toggle favorite

1. Repository inserts or removes an entry in the `favorites` tree.
2. The board refreshes its `favorite_ids` set in memory.

## Query Model

### Recent records

`get_recent(limit)`:

- selects IDs from `time_index`,
- batch-loads only the needed records,
- sorts pinned records first, then newest-first within each group.

This is used during startup to populate the board cache.

### Search and filtering

Search is performed in memory through `filter_records_by_query(...)`.

Current behavior:

- `All`: includes all cached records, but text queries only match text records.
- `Text`: restricts the result set to text records.
- `Image`: returns cached image records and ignores the text query.
- `Favorites`: restricts the result set to favorited records; text queries only match favorited text records.
- Search options support case-sensitive and whole-word matching.

The practical result is that the board searches the visible working set, while the repository remains the source of truth for retention.

## Cleanup Strategy

After inserting a new record, the app calls `cleanup_old_records(max_storage_records)`.

Cleanup rules:

- pinned records are never removed,
- favorited records are never removed,
- oldest unpinned and unfavorited records are deleted first,
- image cleanup also removes the persisted image file and thumbnail.

This gives Ropy two separate retention controls:

- `max_history_records`: UI working set size,
- `max_storage_records`: on-disk retention size.

## Why this design works

1. **Fast startup and rendering**: the board only loads the recent working set.
2. **Cheap recent queries**: `time_index` avoids scanning the whole record tree.
3. **Stable deduplication**: content hashes make updates deterministic.
4. **Safe retention**: favorites and pins protect important entries from cleanup.
5. **Focused search cost**: filtering stays on a bounded in-memory list instead of the whole database.

## Tradeoffs

1. Search scope is limited to the loaded history window rather than the full repository.
2. Foreground and background tasks must coordinate carefully to keep cache and repository state aligned.
3. Shared mutable cache state still relies on `Mutex`, so misuse would show up as UI contention rather than database inconsistency.

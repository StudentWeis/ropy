# Time Index Optimization

## Problem

`get_recent()`, `search()`, and `cleanup_old_records()` all iterate over the entire `clipboard_records` tree, deserializing every record's full JSON (including the potentially large `content` field) just to sort by `created_at` and then truncate.

Since sled keys are content hashes (`u64`), their iteration order is unrelated to time, so we cannot rely on key order for chronological sorting.

## Solution: Secondary Time Index Tree

Add a lightweight sled tree `time_index` that maps composite time-based keys to minimal metadata.

### Key–Value Schema

| Component | Format |
|-----------|--------|
| **Key** | `timestamp_millis: i64 (BE, 8B) ++ id: u64 (BE, 8B)` = 16 bytes |
| **Value** | `pinned: u8 (1B) ++ content_type: u8 (1B)` = 2 bytes |

The main tree `clipboard_records` remains unchanged: `id (u64 BE) → full JSON`.

### `content_type` encoding

| Value | Type |
|-------|------|
| 0 | Text |
| 1 | Image |
| 2 | FilePath |

### `get_recent(limit)` new flow

1. Reverse-iterate `time_index` (newest first), reading only 18 bytes per entry (no JSON parsing).
2. Collect pinned entries first, then unpinned, until total reaches `limit`.
3. Batch-load only the selected `limit` records from `clipboard_records` by ID.
4. Sort the loaded records (pinned first, then by time desc).

### `cleanup_old_records(keep_count)` new flow

1. Forward-iterate `time_index` (oldest first).
2. Skip pinned entries.
3. Delete unpinned entries (from both trees) until `total - removed <= keep_count`.
4. Zero JSON deserialization required.

### `search(keyword)` optimization

The `content_type` byte in the time_index value allows skipping Image/FilePath records during keyword search without deserializing them.

### Write operations

| Operation | Time Index Change |
|-----------|-------------------|
| `save` / `save_image_from_path` | On dedup: delete old time_index entry, insert new one with updated timestamp |
| `save` (new record) | Insert new time_index entry |
| `toggle_pin` | Update time_index value (flip pinned byte) |
| `delete` | Delete time_index entry |
| `clear` | Clear time_index tree |

### Memory comparison (500 records, limit=50)

| Phase | Before | After |
|-------|--------|-------|
| Index scan | 500 × ~300B JSON ≈ 150 KB | 500 × 18B ≈ 9 KB |
| JSON deserialization count | 500 | 50 |
| Final loaded records | 50 | 50 |

### Schema Migration

`SCHEMA_VERSION` bumped from 2 → 3. Old data is cleared on version mismatch (existing behavior).

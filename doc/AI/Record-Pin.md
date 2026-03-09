Record Pinning — Technical Proposal

## Background

Ropy's current history management already has the following basic capabilities:

- Uses content hashes as primary keys to deduplicate identical items.
- Uses `created_at` to maintain recent-use ordering.
- Persists text, images, and file-path records using `sled`.
- Supports search, preview, delete, and quick-select in the UI layer.

On top of this, we need to add a "pin" capability for individual history records so users can keep frequently used records longer without breaking existing deduplication, retrieval, or cleanup strategies.

## Goals

- Allow users to pin and unpin any `record`.
- Pinned records should be shown with priority in lists and search results.
- Pin state must be persisted so it remains after app restarts.
- Compatible with current deduplication: if the same content is copied again, it must not lose its pinned state.
- Compatible with cleanup logic: pinned records should not be deleted during history trimming.

## Non-Goals

- Not introducing multi-level priorities, manual drag-sorting, or custom pin ordering.
- Not changing the database engine or adding a new index structure.

## Design Principles

1. Minimal intrusion: Prefer reusing existing `ClipboardRecord`, `ClipboardRepository`, and `RopyBoard` structures.
2. Data compatibility: New fields must allow smooth reading of older data.
3. Explainable ordering: The ordering users see should be stable and easy to understand.
4. Consistent behavior: Both normal lists and search results follow the same pin rules.

## Data Model Design

Add a boolean field in `ClipboardRecord`:

```rust
pub struct ClipboardRecord {
    pub id: u64,
    pub content: String,
    pub created_at: DateTime<Local>,
    pub content_type: ContentType,
    #[serde(default)]
    pub pinned: bool,
}
```

Design notes:

- Use `pinned: bool` to express pin semantics directly and simply.
- `#[serde(default)]` ensures older serialized data missing the `pinned` field can still deserialize correctly and defaults to `false`.

## Storage Layer Design

### 1. Saving Records

Text and image records default to `pinned = false` on first write.

When duplicates occur, the repository still follows the "overwrite existing record by content hash" strategy and only updates `created_at`, without resetting `pinned`. This ensures:

- Re-copying the same content preserves any existing pin state.
- Deduplication and pinning do not overwrite each other.

### 2. Pin Toggle Interface

The repository should provide `toggle_pin(id)`:

- Read the record by `id`.
- Flip the `pinned` boolean.
- Re-serialize the updated record back into `sled`.

This interface should return the updated `ClipboardRecord` so the UI layer can refresh in-memory state immediately.

### 3. Sorting Rules

Abstract the sorting function as:

1. Pinned records first.
2. Within each group, sort by `created_at` descending.

Example ordering after sorting:

- Pinned record A (newest)
- Pinned record B (older)
- Normal record C (newest)
- Normal record D (older)

Rationale:

- Users can quickly understand "pinned first, then recent within each group."
- Keeps recent-use ordering unchanged within groups—no extra pin-order field required.
- Both search results and the main list can share this sorting logic.

### 4. Search Results

After text matching in `search(keyword)`, apply the same unified sort:

- Show matching pinned records first.
- Then show matching normal records.
- Within each group, sort by `created_at` descending.

This guarantees pinned items remain prioritized in search scenarios as well.

### 5. Cleanup Strategy

`cleanup_old_records(keep_count)` must satisfy:

- Pinned records are excluded from deletion.
- Deletion order still proceeds from the oldest normal records first.

Suggested process:

1. Iterate repository records and extract `(id, created_at, is_pinned)`.
2. Sort by `created_at` ascending.
3. Compute the number to delete: `total - keep_count`.
4. Traverse the sorted list, skip records with `pinned == true`, and delete the oldest normal records until the deletion target is reached or no more deletable records remain.

This yields:

- If there are enough normal records, the total will reduce to `keep_count`.
- If normal records are insufficient, repository size may remain above `keep_count`, but pinned records are always preserved.

## UI Interaction Design

### 1. Presentation

Add two visual elements to history list items:

- A pin indicator (for example, 📌) in the record meta area.
- A pin/unpin action button in the operations area, with a dedicated icon resource such as `assets/icon/pin.svg`.

Button states:

- Pinned: use a highlighted style to indicate active status.
- Unpinned: use a ghost or subdued style to indicate actionability.

### 2. Interaction Flow

When the user clicks the pin button:

1. UI calls `RopyBoard::toggle_record_pin(id)`.
2. `RopyBoard` delegates to the repository `toggle_pin(id)`.
3. Use the returned record to update the in-memory `records` (synchronizing the `pinned` field).
4. Recompute filtered results and notify the UI to refresh.

### 3. Relation to Selection Behavior

The pin semantic is "keep at the top of history and protect from normal history consumption/eviction."

Therefore, selection behavior should adhere to:

- Selecting/confirming a record should not cause pinned records to be deleted by normal consumption flows.
- If the project keeps a behavior like "delete after selection for unpinned items", explicitly skip pinned records in that logic.

## Compatibility Design

### 1. Backward Compatibility

Because the new field uses `#[serde(default)]`:

- Older history records do not require a migration script.
- After upgrading, the app can read old data directly.
- Missing `pinned` fields in old records default to `false`.
- Data serialized with older `Category` enum forms will be ignored for the new field; `pinned` still defaults to `false`.

### 2. Fault Tolerance

In `get_recent()` and `search()`, if a record cannot be deserialized, prefer skipping that record and logging the error rather than failing the whole query. Reasons:

- Improves tolerance during upgrades and error scenarios.
- Prevents a single corrupted record from making the entire history list unusable.

## Test Plan

Recommended unit tests:

- `test_toggle_pin`: verify pin state toggles back and forth.
- `test_pinned_records_appear_first`: verify pinned records are shown before others in the main list.
- `test_multiple_pinned_ordering`: verify multiple pinned records are ordered by `created_at` descending.
- `test_pinned_search`: verify search results respect pinned-first ordering.
- `test_cleanup_skips_pinned`: verify cleanup does not remove pinned records.
- `test_backward_compat_old_category_fields`: verify old `Category`-format data reads with default `pinned = false`.
- Deduplication-related tests: verify re-saving duplicate content preserves the original record's pinned state.

## Performance and Complexity Assessment

- Current sorting and search implementations are based on full traversal and have time complexity O(n log n).
- For small-to-moderate record counts, this approach is simple and maintainable.
- If record volume grows significantly in the future, consider adding secondary indexes or caching sorted results, but avoid premature optimization now.

## Risks and Trade-offs

### 1. Too Many Pins

If a user pins many records, normal records will be crowded out and reduce the visibility of recent items. This initial version accepts that trade-off; subsequent UI improvements (filters/groups) can mitigate it.

### 2. Cleanup Semantics

When many records are pinned, the final stored total may exceed the configured `keep_count`. This is an intentional design choice: protecting pinned records is prioritized over strict compression to `keep_count`.

## Future Enhancements

- Add a "Show only pinned" filter view.
- Add a keyboard shortcut to toggle pin state.
- Add a settings option to control whether selected items are deleted when unpinned (e.g., "after confirm, delete unpinned records" toggle).

Content Type Filter — Technical Proposal

## Background

Ropy stores clipboard records with three content types: `Text`, `Image`, and `FilePath`. Currently the main view shows all types by default, and the search input only matches `Text` records when a query is entered — `Image` records are silently excluded during search.

Users need a way to quickly filter the clipboard history by content type (text or image) without relying on search text. This proposal adds two icon-only toggle buttons next to the search input for filtering.

## Goals

- Allow users to filter clipboard history to show only text records or only image records.
- Filter state is UI-only (not persisted across restarts).
- Filtering works alongside the existing search input: when both a filter and a search query are active, both constraints apply.
- Minimal changes to existing code; reuse existing `ContentType` enum and `gpui-component` button primitives.

## Non-Goals

- No `FilePath` filter button (low usage, not needed now).
- No keyboard shortcuts for filter toggling.
- No persistence of filter state in settings.
- Search input is not disabled when the Image filter is active (query is simply ignored for image records since they cannot be text-searched).

## Design

### 1. State: `ContentFilter` Enum

Add a new enum in `src/gui/board/mod.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ContentFilter {
    #[default]
    All,
    Text,
    Image,
}
```

Add a `content_filter: ContentFilter` field to `RopyBoard`, initialized to `ContentFilter::All`.

### 2. UI Layout

Modify `render_search_input` in `src/gui/board/render.rs` to accept a reference to `RopyBoard` (instead of only `&Entity<InputState>`) and render the search input alongside two icon-only toggle buttons in a horizontal flex row:

```
[ 🔍 Search input ................ ] [T] [🖼]
```

- **Text button (`T` icon)**: toggles `ContentFilter::Text`.
- **Image button (image icon)**: toggles `ContentFilter::Image`.
- Buttons are **mutually exclusive toggles**: clicking an already-active button resets to `All`; clicking the other button switches the filter.
- Active state uses `Button::primary()` style; inactive uses `Button::ghost()` — consistent with the existing pin button pattern.

Icon assets:

- Text filter: `assets/icon/filter-text.svg`
- Image filter: `assets/icon/filter-image.svg`

### 3. Filter Logic

Modify `filter_records_by_query` to accept a `ContentFilter` parameter. The combined behavior:

| `ContentFilter` | Query empty | Query non-empty |
|---|---|---|
| **All** | Return all records | Match query in `Text` records only (current behavior) |
| **Text** | Return only `Text` records | Match query in `Text` records |
| **Image** | Return only `Image` records | Return only `Image` records (query ignored — images cannot be text-searched) |

Updated signature:

```rust
fn filter_records_by_query(
    records: &[ClipboardRecord],
    query: &str,
    filter: ContentFilter,
) -> Vec<ClipboardRecord>
```

`get_filtered_records` passes `self.content_filter` into the filter function.

### 4. Toggle Method

Add a method on `RopyBoard`:

```rust
fn toggle_content_filter(&mut self, target: ContentFilter) {
    if self.content_filter == target {
        self.content_filter = ContentFilter::All;
    } else {
        self.content_filter = target;
    }
}
```

Each button's `on_click` calls this method with the corresponding variant and then `cx.notify()`.

### 5. i18n Keys

Add tooltip keys to all locale files (`assets/locales/*.toml`):

| Key | en | zh-CN | ja |
|---|---|---|---|
| `filter_text` | Filter text | 筛选文本 | テキストをフィルター |
| `filter_image` | Filter images | 筛选图片 | 画像をフィルター |

## Files to Modify

| File | Changes |
|---|---|
| `src/gui/board/mod.rs` | Add `ContentFilter` enum and `content_filter` field; update `filter_records_by_query` signature and logic; update `get_filtered_records` to pass filter; update unit tests |
| `src/gui/board/render.rs` | Change `render_search_input` to accept `&RopyBoard`; render two filter toggle buttons alongside the search input |
| `assets/icon/filter-text.svg` | New icon asset for text filter button |
| `assets/icon/filter-image.svg` | New icon asset for image filter button |
| `assets/locales/en.toml` | Add `filter_text`, `filter_image` keys |
| `assets/locales/zh-CN.toml` | Add `filter_text`, `filter_image` keys |
| `assets/locales/ja.toml` | Add `filter_text`, `filter_image` keys |

## Test Plan

Update and add unit tests in `src/gui/board/mod.rs`:

- **`test_filter_all_no_query`**: `ContentFilter::All` with empty query returns all records (existing behavior, update signature).
- **`test_filter_text_no_query`**: `ContentFilter::Text` with empty query returns only `Text` records.
- **`test_filter_image_no_query`**: `ContentFilter::Image` with empty query returns only `Image` records.
- **`test_filter_text_with_query`**: `ContentFilter::Text` with a query returns matching `Text` records.
- **`test_filter_image_ignores_query`**: `ContentFilter::Image` with a query still returns all `Image` records (query ignored).
- **`test_filter_all_with_query`**: `ContentFilter::All` with a query returns matching `Text` records only (existing behavior).
- **`test_toggle_content_filter`**: toggling the same filter twice returns to `All`; toggling a different filter switches.

## Compatibility

- No data model changes; no migration needed.
- No settings persistence changes.
- Filter state resets to `All` on each app launch — intentional, as filter is a transient UI concern.

## Risks and Trade-offs

### Search + Image Filter

When the Image filter is active, the search input remains visible but its query is ignored. This is a minor UX inconsistency, but avoids the complexity of conditionally hiding or disabling the search input. Users can clear the search text themselves if it causes confusion.

### Future: FilePath Filter

If `FilePath` records become more common, a third filter button can be added following the same pattern. The `ContentFilter` enum is easy to extend.

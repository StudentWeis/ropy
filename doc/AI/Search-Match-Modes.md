Search Match Modes — Technical Proposal

## Background

Ropy currently supports only case-insensitive substring search for text clipboard records. This is implemented in the board filtering path and works well for broad lookup, but it does not support stricter matching workflows.

Users now need two additional capabilities:

- Case-sensitive search
- More precise matching modes: whole-word match and exact match

The feature should integrate with the existing search box and content-type filter without introducing heavy configuration or persistent settings.

## Goals

- Add a case-sensitive toggle for text search.
- Add three text match modes:
  - Contains
  - Whole word
  - Exact
- Keep the existing content-type filter behavior unchanged.
- Keep search options UI-only and non-persistent.
- Preserve current default behavior on startup.

## Non-Goals

- No natural-language segmentation for CJK text.
- No regex search mode.
- No settings-page persistence.
- No keyboard shortcut changes in this iteration.

## Accepted Product Decisions

- Whole-word matching is designed primarily for English/code-like token boundaries.
- Exact match uses strict full-string equality and does not trim whitespace.

## Design

### 1. Search State

Add a new match mode enum and search options struct in `src/gui/board/mod.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SearchMatchMode {
    #[default]
    Contains,
    WholeWord,
    Exact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SearchOptions {
    pub match_mode: SearchMatchMode,
    pub case_sensitive: bool,
}
```

Add `search_options: SearchOptions` to `RopyBoard`, initialized with the default value.

### 2. Match Semantics

Text matching is handled by a dedicated helper:

```rust
fn text_matches_query(content: &str, query: &str, options: SearchOptions) -> bool
```

Behavior matrix:

| Match mode | Case-sensitive | Rule |
|---|---|---|
| Contains | Off | case-insensitive substring match |
| Contains | On | case-sensitive substring match |
| WholeWord | Off | case-insensitive token-boundary match |
| WholeWord | On | case-sensitive token-boundary match |
| Exact | Off | case-insensitive full-string equality |
| Exact | On | case-sensitive full-string equality |

### 3. Whole-Word Definition

Whole-word matching checks whether the query appears as a standalone token.

Boundary rule:

- Token characters are `char::is_alphanumeric()` or `_`
- A match is valid only when both sides of the matched slice are either:
  - missing (start/end of string), or
  - non-token characters

Examples:

- Query `hello` matches `hello world`
- Query `hello` matches `(hello)`
- Query `hello` does not match `hello_world`
- Query `he` does not match `hello`

This intentionally does not attempt full natural-language word segmentation for Chinese or Japanese text.

### 4. Filter Integration

Update the board filter entry point to accept search options:

```rust
fn filter_records_by_query(
    records: &[ClipboardRecord],
    query: &str,
    filter: ContentFilter,
    options: SearchOptions,
) -> Vec<ClipboardRecord>
```

Existing content-type behavior remains unchanged:

- `ContentFilter::All`: search applies only to `Text` records when query is non-empty
- `ContentFilter::Text`: search applies to `Text` records only
- `ContentFilter::Image`: query is ignored

### 5. UI

Update the search row in `src/gui/board/render.rs`:

- Add a case-sensitive toggle button with label `Aa`
- Add a match-mode cycle button with short labels:
  - `.*` for contains
  - `W` for whole word
  - `=` for exact
- Keep the existing text/image filter buttons unchanged

The row becomes:

```text
[ search input........................ ] [Aa] [.*|W|=] [Text] [Image]
```

UI behavior:

- `Aa` toggles `case_sensitive`
- Match-mode button cycles `Contains -> WholeWord -> Exact -> Contains`
- Active case-sensitive state uses `Button::primary()`
- Match-mode button always reflects the current mode label

### 6. i18n

Add new locale keys:

- `search_case_sensitive`
- `search_match_mode`
- `search_match_contains`
- `search_match_whole_word`
- `search_match_exact`

The match-mode button tooltip is composed as:

```text
{search_match_mode}: {current_mode_label}
```

## Files to Modify

| File | Changes |
|---|---|
| `src/gui/board/mod.rs` | Add search option types, toggle helpers, matching helpers, filter updates, and tests |
| `src/gui/board/render.rs` | Add the new search-option buttons |
| `assets/locales/en.toml` | Add search-related locale keys |
| `assets/locales/zh-CN.toml` | Add search-related locale keys |
| `assets/locales/ja.toml` | Add search-related locale keys |

## Test Plan

Add unit tests covering:

- contains match, case-insensitive
- contains match, case-sensitive
- whole-word match, case-insensitive
- whole-word match, case-sensitive
- exact match, case-insensitive
- exact match, case-sensitive
- whole-word rejects partial token matches
- exact match rejects longer content
- image filter still ignores query for all search modes
- match-mode cycling order
- case-sensitive toggle behavior

## Risks and Trade-offs

### Whole-Word Matching in CJK Text

This proposal does not implement language-aware tokenization. For CJK text, whole-word mode behaves according to generic Unicode alphanumeric boundaries, which is acceptable for the first iteration but should not be marketed as linguistic segmentation.

### UI Density

Adding two more controls beside the search field increases horizontal density. Using concise labels plus tooltips keeps the first iteration lightweight and avoids adding a popover component.

## Compatibility

- No repository or schema changes
- No configuration migration
- Default startup behavior remains case-insensitive contains match

# Clear Confirm Dialog

## Background

Ropy's header bar contains a "Clear All" button (`clear-button`) that immediately invokes `clear_history()` and `clear_last_copy_state()`, which in turn calls `ClipboardRepository::clear()` to wipe all records, time-index entries, and image files from the database.

Because the button sits in the header alongside other frequently used controls, accidental clicks can permanently destroy the user's entire clipboard history with no way to recover. This proposal adds a lightweight confirmation overlay to prevent unintended data loss.

## Goals

- Prevent accidental clearing of all clipboard records by requiring explicit user confirmation.
- Keep the interaction lightweight: a small centered overlay, not a full-screen panel.
- Use a destructive button style to visually communicate the danger of the action.
- Support all three existing languages (English, 简体中文, 日本語).

## Non-Goals

- Do not display the exact number of records to be cleared.
- Do not introduce an undo/trash mechanism.
- Do not change the underlying `ClipboardRepository::clear()` behavior.
- Do not add a "don't ask again" toggle in this iteration.

## UX Design

### Trigger

When the user clicks the "Clear All" button in the header, instead of immediately clearing data, the app shows a confirmation overlay.

### Overlay Layout

The overlay consists of two layers:

1. **Backdrop**: A semi-transparent dark layer (`rgba(0, 0, 0, 0.3)`) covering the entire window. Clicking the backdrop dismisses the dialog (equivalent to cancel).
2. **Dialog card**: A centered rounded card with the following content from top to bottom:
   - **Title**: A bold heading (e.g., "Clear All Records" / "清空所有记录").
   - **Message**: A short warning explaining the consequence (e.g., "This will permanently delete all saved clipboard records. This action cannot be undone." / "将永久删除所有已保存的剪贴板记录，此操作不可撤销。").
   - **Action buttons** (right-aligned row):
     - **Cancel button**: Ghost style, dismisses the dialog without any side effects.
     - **Confirm button**: Destructive/danger style (red-tinted or `danger` variant) to emphasize the irreversible nature.

### Keyboard Interaction

- **Esc**: Dismisses the dialog (same as cancel). This integrates with the existing `on_hide_action` cascade.
- **Enter**: Should **not** confirm the action to avoid accidental double-tap. The user must click the confirm button explicitly.

### Visual Reference

```
┌──────────────────────────────────┐
│  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │  ← backdrop
│  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │
│  ░░░┌────────────────────┐░░░░░  │
│  ░░░│  ⚠ Clear All       │░░░░░  │  ← title
│  ░░░│                    │░░░░░  │
│  ░░░│  This will delete  │░░░░░  │  ← message
│  ░░░│  all saved records.│░░░░░  │
│  ░░░│  Cannot be undone. │░░░░░  │
│  ░░░│                    │░░░░░  │
│  ░░░│    [Cancel] [Clear] │░░░░░  │  ← buttons
│  ░░░└────────────────────┘░░░░░  │
│  ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░  │
└──────────────────────────────────┘
```

## State Design

Add a single boolean field to `RopyBoard`:

```rust
pub(crate) show_clear_confirm: bool,
```

Default value: `false`.

No additional structs, entities, or async state are needed.

## Implementation Plan

### 1. `src/gui/board/mod.rs` — Add state field

Add `show_clear_confirm: bool` to the `RopyBoard` struct, initialized to `false` in the constructor.

In `Render::render`, add a rendering branch: when `show_clear_confirm` is `true`, overlay the confirmation dialog on top of the main content. This should be rendered **after** the main body (similar to how notifications are layered) using `deferred` + absolute positioning to ensure it sits above all other content.

### 2. `src/gui/board/render.rs` — Modify clear button and add dialog renderer

**Modify `create_clear_button`**: Change the `on_click` handler from directly calling `clear_history()` to setting `show_clear_confirm = true` and calling `cx.notify()`.

**Add `render_clear_confirm_overlay` function**: Renders the two-layer overlay:

- Outer `div`: absolute, full-size, semi-transparent background, with an `on_click` handler that sets `show_clear_confirm = false` (cancel on backdrop click).
- Inner card `div`: centered (using flex centering on the outer div), white/themed background, rounded corners, padding, containing:
  - Title text using `board.i18n.t("clear_confirm_title")`.
  - Message text using `board.i18n.t("clear_confirm_message")`.
  - Button row:
    - Cancel `Button`: ghost style, label from `board.i18n.t("clear_confirm_cancel")`, sets `show_clear_confirm = false`.
    - Confirm `Button`: danger style, label from `board.i18n.t("clear_confirm_button")`, calls `clear_history()` + `clear_last_copy_state()` + sets `show_clear_confirm = false`.

Use `stop_propagation` on the inner card's `on_mouse_down` to prevent backdrop click-through.

### 3. `src/gui/board/actions.rs` — Esc key handling

In `on_hide_action`, add a new early-return branch before the existing `show_about` / `show_help` checks:

```rust
if self.show_clear_confirm {
    self.show_clear_confirm = false;
    cx.notify();
    return;
}
```

This ensures pressing Esc dismisses the confirmation dialog without hiding the window.

### 4. `assets/locales/*.toml` — i18n keys

Add the following keys to all three locale files:

| Key | en | zh-CN | ja |
|---|---|---|---|
| `clear_confirm_title` | Clear All Records | 清空所有记录 | すべての記録を消去 |
| `clear_confirm_message` | This will permanently delete all saved clipboard records. This action cannot be undone. | 将永久删除所有已保存的剪贴板记录，此操作不可撤销。 | 保存されたすべてのクリップボード記録が完全に削除されます。この操作は取り消せません。 |
| `clear_confirm_cancel` | Cancel | 取消 | キャンセル |
| `clear_confirm_button` | Clear | 清空 | 消去 |

### 5. `src/i18n/translations.rs` — Register new keys

Add the four new keys to the translation key registry so the i18n system can resolve them. Follow the existing pattern used by other keys in this file.

## Files Touched

| File | Change |
|---|---|
| `src/gui/board/mod.rs` | Add `show_clear_confirm` field; render overlay in `Render::render` |
| `src/gui/board/render.rs` | Modify `create_clear_button` click handler; add `render_clear_confirm_overlay` |
| `src/gui/board/actions.rs` | Add Esc handling for `show_clear_confirm` in `on_hide_action` |
| `assets/locales/en.toml` | Add 4 i18n keys |
| `assets/locales/zh-CN.toml` | Add 4 i18n keys |
| `assets/locales/ja.toml` | Add 4 i18n keys |
| `src/i18n/translations.rs` | Register 4 new translation keys |

## Edge Cases

- **Dialog open + window loses focus**: The dialog state persists. When the window is re-activated via hotkey, `on_active_action` should reset `show_clear_confirm = false` to avoid showing a stale dialog.
- **Dialog open + settings/about/help**: These panels are mutually exclusive with the main view. The clear button is only visible on the main view, so `show_clear_confirm` should not conflict with panel states. However, `on_active_action` resetting the flag provides a safety net.
- **Empty history**: The clear button is always visible. Confirming on an empty history is a no-op at the repository level and is harmless.

## Validation

After implementation, run:

- `cargo test` — ensure no regressions.
- `python script/check_i18n.py` — ensure all locale files have matching keys.
- `./script/precheck.sh` — full quality check.

# GPUI Best Practices Review

A systematic review of the project's GPUI usage against established best practices from each GPUI skill domain.

## Improvement Opportunities

### 1. Add `key_context` for Action Isolation

**Priority**: High
**Effort**: Low
**Skill**: gpui-action

**Current state**: All key bindings use `None` context (global scope):

```rust
// app.rs — bind_application_keys()
KeyBinding::new("escape", Hide, None),
KeyBinding::new("up", SelectPrev, None),
KeyBinding::new("enter", ConfirmSelection, None),
```

The render method also lacks `key_context`:

```rust
// render.rs
div()
    .track_focus(&self.focus_handle)
    .on_action(cx.listener(Self::on_select_prev))
```

**Risk**: As the project grows with more panels/dialogs, global shortcuts will conflict. For example, `up`/`down` should not trigger `SelectPrev`/`SelectNext` inside the settings panel.

**Recommendation**:

```rust
const BOARD_CONTEXT: &str = "Board";

// Bind with context
KeyBinding::new("up", SelectPrev, Some(BOARD_CONTEXT)),
KeyBinding::new("down", SelectNext, Some(BOARD_CONTEXT)),

// Set context on element
div()
    .key_context(BOARD_CONTEXT)
    .track_focus(&self.focus_handle)
```

### 3. Replace Boolean Flags with Panel Enum

**Priority**: Medium
**Effort**: Medium
**Skill**: gpui-patterns

**Current state**: `RopyBoard` uses multiple boolean flags to track which panel is visible:

```rust
pub(crate) show_settings: bool,
pub(crate) show_about: bool,
pub(crate) show_help: bool,
```

This allows invalid states (e.g., `show_settings = true` AND `show_about = true` simultaneously).

**Recommendation**: Use an enum to make illegal states unrepresentable:

```rust
enum ActivePanel {
    ClipboardList,
    Settings,
    About,
    Help,
}
```

### 4. Introduce Custom Events for Cross-Component Communication

**Priority**: Medium
**Effort**: Medium
**Skill**: gpui-event

**Current state**: Zero usage of `cx.emit()` / `cx.subscribe()` for custom events. Cross-component communication relies entirely on direct `entity.update()` calls:

```rust
// app.rs
board.update(cx, |board, cx| {
    board.refresh_records_from_repository(cx);
    cx.notify();
});
```

**Recommendation**: Define domain events to decouple components:

```rust
#[derive(Clone)]
enum BoardEvent {
    RecordsUpdated,
    SettingsChanged,
}
```

This reduces coupling between `app.rs` and `RopyBoard`'s internal methods.

### 5. Store Download Task Instead of Detaching

**Priority**: Medium
**Effort**: Low
**Skill**: gpui-async

**Current state**: `updater_ui.rs` detaches the download task, making it impossible to cancel:

```rust
cx.spawn(async move |this, cx| {
    // ... download progress loop ...
})
.detach();
```

**Recommendation**: Store the task handle in `UpdateManager`:

```rust
struct UpdateManager {
    status: UpdateStatus,
    _download_task: Option<Task<()>>,
}
```

When the task is dropped (e.g., user navigates away), it is automatically cancelled.

### 6. Reduce `RopyBoard` Responsibilities

**Priority**: Low
**Effort**: High
**Skill**: gpui-patterns, gpui-entity

**Current state**: `RopyBoard` has 20+ fields and manages clipboard records, search/filtering, settings, about/help panels, update management, hotkey recording, window pinning, and clear confirmation.

**Recommendation**: Extract sub-components as independent entities following the Container/Presenter pattern:

- `SettingsPanel` as `Entity<SettingsPanel>`
- `UpdateManager` as `Entity<UpdateManager>`
- Hotkey recording logic into a dedicated handler entity

### 7. Add GUI Component Tests

**Priority**: Low
**Effort**: Medium
**Skill**: gpui-test

**Current state**: Tests exist for pure logic (`hotkey.rs`, `paste.rs`, `constants.rs`) but GUI components (`RopyBoard`, action handlers, render logic) have no test coverage.

**Recommendation**: Add tests for:

- `on_select_prev` / `on_select_next` boundary conditions
- `on_delete_record` index clamping after deletion
- `filter_and_sort_record_indices` correctness
- Settings panel state transitions

Per gpui-test guidelines, pure logic tests can use `#[test]`; entity interaction tests should use `#[gpui::test]`.

### 8. Add Visual Focus Indicators

**Priority**: Low
**Effort**: Low
**Skill**: gpui-focus-handle

**Current state**: `track_focus` is used but no visual feedback is provided based on focus state.

**Recommendation**:

```rust
let is_focused = self.focus_handle.is_focused(cx);

div()
    .track_focus(&self.focus_handle)
    .when(is_focused, |el| {
        el.border_color(cx.theme().ring)
    })
```

For a popup-style window this is low priority, but becomes important if the app evolves to have multiple focusable regions.

---

## Summary

| Dimension | Rating | Key Finding |
|---|---|---|
| **gpui-action** | ⭐⭐⭐ | Good naming, missing `key_context` and manual key dispatch |
| **gpui-async** | ⭐⭐⭐⭐⭐ | Exemplary background/foreground separation |
| **gpui-context** | ⭐⭐⭐⭐⭐ | Correct usage throughout |
| **gpui-element** | ⭐⭐⭐⭐ | Not used (not needed) — appropriate |
| **gpui-entity** | ⭐⭐⭐⭐ | Weak refs correct, component decomposition possible |
| **gpui-event** | ⭐⭐⭐ | Custom events unused, higher coupling |
| **gpui-focus-handle** | ⭐⭐⭐⭐ | Solid management, missing visual indicators |
| **gpui-global** | ⭐⭐⭐⭐⭐ | Clean separation of global vs. entity state |
| **gpui-patterns** | ⭐⭐⭐ | `RopyBoard` has too many responsibilities |
| **gpui-test** | ⭐⭐ | GUI component test coverage insufficient |

### Suggested Priority Order

1. **High**: Add `key_context` isolation — small change, prevents future conflicts
2. **Medium**: Introduce `ActivePanel` enum — eliminates invalid states
3. **Medium**: Store download task handle — enables cancellation
4. **Medium**: Add custom events for cross-component communication
5. **Low**: Decompose `RopyBoard` into sub-components
6. **Low**: Add GUI component tests
7. **Low**: Add visual focus indicators

## Selection Reveal State Analysis

### Context

`GridRevealState` was introduced while splitting `RopyBoard` state into smaller groups. It replaced the previous boolean field `grid_auto_reveal_suppressed`.

The current model is:

- `GridRevealState::Auto`: grid mode is allowed to scroll the selected item into view.
- `GridRevealState::Suppressed`: grid mode should not automatically scroll the selected item into view.

The only suppressing call site is in the masonry grid scroll container. When the user scrolls the grid with the wheel, the board calls `suppress_grid_auto_reveal()`.

The central question is whether this is a real domain state or a workaround for reveal side effects happening at the wrong time.

### First principles

There are two independent concepts:

- Selection: `selected_index` represents the item selected by keyboard/navigation logic.
- Viewport: the scroll position represents what the user is currently looking at.

User scrolling should not change `selected_index`. That is true for both list and grid layouts.

Therefore, if an automatic reveal can pull the viewport back to `selected_index` after the user scrolls, that issue is not inherently grid-specific. It is a consequence of when `reveal_selected_record()` is called.

### When reveal is appropriate

Revealing the selected item is appropriate when the application intentionally changes, restores, or re-contextualizes selection.

Examples:

- Keyboard navigation changes `selected_index`.
- Search/filtering changes the visible result set and clamps or resets selection.
- Switching layout should preserve the selected item and bring it into the new layout context.
- Activating the window may intentionally focus the user's current selection.

In these cases, selection is the user's primary intent, so moving the viewport to selection is expected.

### When reveal is not appropriate

Revealing the selected item is not appropriate when selection did not change and the user's current viewport is more important.

Examples:

- The user manually scrolls.
- A background clipboard event refreshes records without a user navigation intent.
- A normal re-render occurs.
- Repository refresh updates backing data but should not reinterpret viewport intent.

In these cases, revealing selection is an accidental side effect.

### Assessment of `GridRevealState`

`GridRevealState` encodes a historical fact: the user recently scrolled the grid, so future automatic reveal should be suppressed.

That makes it suspicious as core UI state. It does not describe the user's current task directly; it compensates for reveal calls that may be happening from overly broad refresh paths.

It is also asymmetric:

- `selected_index` is independent from scrolling in both list and grid layouts.
- The same theoretical auto-reveal problem can exist in both layouts.
- Only grid has an explicit wheel handler that suppresses reveal.

This suggests the state is coupled to the current masonry implementation rather than to a stable product concept.

### Recommended direction

Prefer making reveal an explicit event-scoped side effect instead of storing a persistent reveal mode.

Recommended model:

- Selection-changing operations should call `reveal_selected_record()` directly.
- Layout activation or layout switching can call `reveal_selected_record()` when that is the intended behavior.
- Data refresh paths should take an explicit `reveal_selection` decision and default to preserving the current viewport.
- User scrolling should not need to mutate board state just to prevent future reveal.

Under this model, `GridRevealState` and `suppress_grid_auto_reveal()` can likely be removed.

### Practical refactoring plan

1. Audit all `force_reveal_selected_record()` call sites.
2. Classify each call site by user intent:
   - Selection changed: reveal is appropriate.
   - Layout/window context changed: reveal may be appropriate.
   - Data refreshed only: reveal should usually be avoided.
3. Replace broad reveal behavior with explicit call-site decisions.
4. Remove `GridRevealState` once no refresh path needs a suppression guard.
5. If list and grid need different viewport preservation behavior later, model that behavior at the scroll container boundary instead of storing a global board-level suppression flag.

### Conclusion

`GridRevealState` is probably not necessary as durable `RopyBoard` state.

The cleaner design is to treat reveal as an intentional side effect of selection/navigation events. If a reveal is undesirable after user scrolling, the better fix is to avoid calling reveal from that later path, not to remember that the user scrolled and suppress reveal afterward.

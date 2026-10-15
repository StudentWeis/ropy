# Focus and keyboard navigation

**Contents:** [Retained focus](#retained-focus) · [Focus events](#focus-events) · [Navigation and restoration](#navigation-and-restoration)

## Retained focus

Create a FocusHandle in the entity that owns keyboard interaction, then attach
it with `.track_focus(&self.focus_handle)`. Put the relevant `key_context` and
`on_action` handlers on that focused region. Do not create a fresh handle every
render or request focus unconditionally from render.

In the locked `gpui-pre` 0.3.9 APIs:

| Operation | Call |
| --- | --- |
| Create | `cx.focus_handle()` |
| Request focus | `handle.focus(window, cx)` |
| Check exact target | `handle.is_focused(window)` |
| Clear window focus | `window.blur(cx)` |
| Move through tab order | `window.focus_next(cx)` / `window.focus_prev(cx)` |

Focus queries take a Window, not an App/Context. An action method passed through
`cx.listener` includes `&mut Window`; see [action handlers](action.md).
The [versioned Window source](https://docs.rs/crate/gpui-pre/0.3.9/source/src/window.rs)
contains FocusHandle and the window focus operations.

## Focus events

Register callbacks on the owning Context and retain their Subscriptions.
`cx.on_focus(&handle, window, |this, window, cx| ...)` and `on_blur` observe
the exact handle. `on_focus_in` includes descendants; `on_focus_out` also
passes a `FocusOutEvent` before Window. These are Context methods, not
`.on_focus(...)` or `.on_blur(...)` builders on a div.

For example, inside a constructor that stores `_subscriptions`:

```rust
let focus_handle = cx.focus_handle().tab_stop(true);
let subscription = cx.on_focus(&focus_handle, window, |_, _, cx| cx.notify());
// Store both focus_handle and subscription in the new entity.
```

See the [locked focus subscription signatures](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/context.rs).
Use the framework's focus styles and semantic theme roles; do not invent a
`focused_background` theme field. The normative
[focus guide](../coding-guides.md#events-actions-and-focus) defines the visible
focus and overlay requirements.

## Navigation and restoration

A tracked handle is a Tab stop only if the handle opts in:
`cx.focus_handle().tab_stop(true)` (or its `tab_index` builder). Element-level
Tab settings do not replace the settings on a handle supplied to `track_focus`.
For value-like controls, keyed state can retain the handle across consecutive
frames; stable IDs are essential. Prefer existing Button/Input/Dialog behavior
over reimplementing keyboard activation or a modal focus trap.

When a focused element disappears, restore a still-rendered ancestor or an
explicit root target. Register `cx.on_focus_lost(window, |this, window, cx| ...)`
once in the owning view, retain the Subscription, obtain
`window.focus_lost_restore_target(cx)`, and focus the chosen handle with
`target.focus(window, cx)`. Track the root handle on the region with its key
context so keyboard dispatch has a valid fallback.

Opening and dismissing overlays must transfer and restore focus in order,
including nested overlays. Test Tab/Shift-Tab, keyboard activation, disabled
controls, dismissal, and removal of the focused item through real input; see
[UI integration testing](test.md).

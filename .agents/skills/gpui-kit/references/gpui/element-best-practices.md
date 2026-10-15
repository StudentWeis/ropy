# Reviewing custom elements

**Contents:** [Lifecycle](#lifecycle) · [Performance](#performance) · [Behavior and tests](#behavior-and-tests)

## Lifecycle

Use the [Element API contracts](element-api.md) and the normative
[phase guide](../coding-guides.md#understand-gpuis-phases-and-contexts).
Check that:

- Layout is requested before its bounds are read, and measurement uses the
  resolved available space rather than assuming the window width is the control width.
- Each child and its layout/prepaint state survive through the same frame's phases.
- Selection, scroll position, tasks, and reusable caches have a retained owner.
- Stable IDs identify repeated domain objects; recreated element fields are
  not mistaken for state retained by those IDs.
- `'static` event callbacks capture handles/owned data, not frame borrows.

## Performance

Keep paint focused on drawing and frame event registration. Avoid repeated
expensive work, but measure before adding a cache or declaring all allocations
forbidden. Cache keys must cover content, width, font metrics, rem size, and
other inputs used by the calculation. Place a cross-frame cache in retained
state and define its invalidation path.

Use existing virtualization for unbounded collections. A hand-written visible
range needs correct measurement, scroll-to-item, focus, and invalidation
behavior; a short loop is not a substitute for those contracts.

## Behavior and tests

Insert hitboxes in prepaint. Honor event phases and hit testing, and stop
propagation only when an ancestor must not handle the same interaction. Render
focus and disabled states and supply keyboard/accessibility behavior.

Test layout through a rendered headless window and assert resolved bounds.
Calling request_layout and immediately reading layout_bounds skips computation
and is not a valid geometry test. Test interaction through pointer/keyboard
input and assert retained state after redraw, including reordered or removed
items. Use [the existing UI integration workflow](test.md); do not build a
parallel harness merely to exercise illustrative documentation.

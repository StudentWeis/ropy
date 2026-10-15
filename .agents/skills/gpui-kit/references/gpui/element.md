# Custom elements

**Contents:** [Choose the abstraction](#choose-the-abstraction) · [Rendering phases](#rendering-phases) · [Retained behavior](#retained-behavior) · [References](#references)

## Choose the abstraction

Use RenderOnce for supplied, value-like presentation and an Entity-backed
Render view for behavior that survives frames. Use the low-level Element trait
only when layout, measurement, or painting cannot be expressed by existing
components. A custom canvas can provide small drawing hooks without duplicating
an entire Element implementation.

These notes describe the locked `gpui-kit` 0.7.3 / `gpui-pre` 0.3.9 sources.
They replace illustrative implementations that were not runnable applications.
Read the normative [choice of unit](../coding-guides.md#choose-the-right-unit)
and [layout guide](../coding-guides.md#layout-measurement-and-scrolling) first.

## Rendering phases

| Phase | Work | State passed onward |
| --- | --- | --- |
| `request_layout` | Build layout nodes and request measurement | `RequestLayoutState` |
| `prepaint` | Use resolved bounds, place children, insert hitboxes | `PrepaintState` |
| `paint` | Draw and register the frame's input handlers | Both previous states are available |

Requesting a LayoutId does not resolve its bounds. The framework computes
layout before prepaint; do not query a newly requested child's layout bounds
inside request_layout to implement masonry or row sizing. Use the resolved
bounds supplied to prepaint or an appropriate measured-layout callback.

Keep the same child elements and their phase state through all three phases.
Do not recreate a StyledText or AnyElement at paint time after laying out a
different instance. AnyElement's wrapper methods differ from the Element trait
methods; inspect their actual signatures before forwarding children.

## Retained behavior

Elements and their associated phase state describe a frame. Persistent
selection, scroll position, caches, and async tasks need an Entity, an existing
handle, or explicitly keyed element state. An ElementId supplies identity;
it does not preserve arbitrary fields of a newly constructed element.

Input handlers registered during paint are `'static`. They cannot borrow
`&mut self`, a phase-state reference, Window, or App from the current call.
Capture owned callbacks/handles and update the retained owner when dispatched.
Insert hitboxes during prepaint and capture the owned hitbox for hit testing.
Honor dispatch phase and stop propagation only when the behavior needs it.

## References

- [API contracts](element-api.md): exact trait/wrapper distinction and versioned sources.
- [Patterns](element-patterns.md): composition, text, input, and scrolling ownership.
- [Source examples](element-examples.md): implementations maintained with the framework.
- [Review checklist](element-best-practices.md): state, geometry, event and test checks.
- [Advanced mechanisms](element-advanced.md): custom measurement, caching and async work.

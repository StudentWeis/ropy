# Element composition patterns

**Contents:** [Composition](#composition) · [Interaction](#interaction) · [Scrolling](#scrolling)

## Composition

Prefer the existing component's behavior and a RenderOnce wrapper for custom
presentation. Use the [Element lifecycle](element.md) when actual custom
measurement or painting is required.

For text, retain the same shaped/text element and its layout state from layout
through paint; do not instantiate a second text element in paint. Text width,
font, rem size, and content changes invalidate measurements. For containers,
retain the children and forward each phase through the correct concrete or
AnyElement API. Follow the [versioned text implementation](https://docs.rs/crate/gpui-pre/0.3.9/source/src/elements/text.rs)
and [Div implementation](https://docs.rs/crate/gpui-pre/0.3.9/source/src/elements/div.rs)
instead of copying an abbreviated custom container template.

## Interaction

Use Button, Input, Select, and other semantic controls for their standard
behavior. A custom low-level interaction still needs hit testing, focus,
keyboard activation, disabled handling, accessible semantics, and dispatch
policy. A mouse-up listener alone is not a complete button contract.

Create hitboxes in prepaint. During paint, capture owned handles and callbacks
for the frame's `'static` listeners. A borrowed Box callback cannot be made
owned by calling clone on its reference; use an intentionally shared callback
or move an owned value. Persistent selection belongs to an Entity/keyed state,
not the transient element assembled by render.

## Scrolling

Give scrolling one owner and reuse existing scroll handles or virtualized
components. A simple wheel callback plus a stored offset does not implement
clipping, keyboard scroll-to-item, focus visibility, variable-height rows,
measurement invalidation, or platform scroll behavior. Keep row identity
separate from visible indexes and use stable domain IDs.

The normative [layout and scrolling guide](../coding-guides.md#layout-measurement-and-scrolling)
and [large-data guide](../coding-guides.md#lists-tables-and-large-data) define
the ownership rules. Use [UI tests](test.md) to verify actual layout and input.

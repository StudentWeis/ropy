# Custom element source examples

**Contents:** [Maintained examples](#maintained-examples) · [Adapting an implementation](#adapting-an-implementation)

## Maintained examples

Use these complete framework implementations from the locked `gpui-pre`
0.3.9 source, rather than standalone tutorials with omitted trait methods or
invented lifecycle calls:

| Mechanism | Source |
| --- | --- |
| Small custom drawing with prepaint state | [Canvas](https://docs.rs/crate/gpui-pre/0.3.9/source/src/elements/canvas.rs) |
| Shaping, measuring, and painting the same text state | [Text elements](https://docs.rs/crate/gpui-pre/0.3.9/source/src/elements/text.rs) |
| Child lifecycle, hitboxes, input and style composition | [Div](https://docs.rs/crate/gpui-pre/0.3.9/source/src/elements/div.rs) |
| Concrete Element and type-erased wrapper contracts | [Element and AnyElement](https://docs.rs/crate/gpui-pre/0.3.9/source/src/element.rs) |

These are framework source references, not code to paste wholesale into an
application. Internal imports and supporting types belong to that crate.
For an application-owned view and retained component state, use the
[application recipes](../recipes.md).

## Adapting an implementation

Read the relevant trait plus the source's construction and ownership paths.
Preserve the same children and phase state between request_layout, prepaint,
and paint. Use resolved geometry only after layout, and keep persistent input
state in an Entity or keyed state. Event closures must own their captures.

Choose the smallest abstraction that meets the requirement: a semantic
component first, then a RenderOnce wrapper, a canvas for drawing, and a custom
Element only when necessary. Validate the application behavior with the
[existing testing workflow](test.md); a source excerpt alone is not evidence
that an adapted control compiles or handles user input correctly.

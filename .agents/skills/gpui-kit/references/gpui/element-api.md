# Element API contracts

**Contents:** [Trait and wrappers](#trait-and-wrappers) · [Window operations](#window-operations) · [Identity and state](#identity-and-state)

## Trait and wrappers

The authoritative API is the [gpui-pre 0.3.9 Element source](https://docs.rs/crate/gpui-pre/0.3.9/source/src/element.rs),
selected by this repository's `gpui-kit` 0.8.0 dependency. Element has **two**
associated types, RequestLayoutState and PrepaintState. Its required methods
are id, source_location, request_layout, prepaint, and paint; it also supplies
optional hooks such as accessibility behavior. IntoElement is a separate
trait that exposes the concrete Element type.

| Method | Inputs after `&mut self` | Result |
| --- | --- | --- |
| `Element::request_layout` | optional global/inspector IDs, Window, App | `(LayoutId, RequestLayoutState)` |
| `Element::prepaint` | IDs, resolved Bounds, mutable layout state, Window, App | `PrepaintState` |
| `Element::paint` | IDs, resolved Bounds, mutable layout/prepaint states, Window, App | `()` |
| `AnyElement::request_layout` | Window, App | `LayoutId` |
| `AnyElement::prepaint` | Window, App | `Option<FocusHandle>` |
| `AnyElement::paint` | Window, App | `()` |

The type-erased wrapper owns its child's lifecycle state. Do not destructure
its request_layout result as a tuple or pass bounds/IDs to its ordinary
prepaint/paint methods. For explicitly positioned children, read the source of
prepaint_at/prepaint_as_root and the owning container before using those APIs.

## Window operations

The [versioned Window source](https://docs.rs/crate/gpui-pre/0.3.9/source/src/window.rs)
defines request_layout, request_measured_layout, compute_layout, layout_bounds,
insert_hitbox, on_mouse_event, and on_key_event. Read their phase requirements:
layout_bounds is meaningful only after computation, hitboxes belong in
prepaint, and frame input handlers are installed during paint.

`on_mouse_event` receives a `'static` FnMut callback with
`(&Event, DispatchPhase, &mut Window, &mut App)`; the key callback is Fn.
Capture owned data or handles, not an Element's current `&mut self`.
Painting order is explicit: a parent's background normally precedes its
children, while overlays follow them. There is no general rule to paint every
child before all parent content.

## Identity and state

Element::id keys identity across frames and scopes child IDs; it is not merely
an inspector label. GlobalElementId is assembled from the window's element
identity path. Use [stable domain IDs](element-id.md), not list positions.

Window::with_element_state takes a GlobalElementId and a closure that receives
`Option<S>` and returns `(result, S)`. The returned state is retained for the
next frame. Window::use_keyed_state returns an Entity for state retained while
the key remains rendered in consecutive frames. Arbitrary Element fields and
phase-state associated types do not provide durable application ownership.
See [state ownership](../coding-guides.md#state-ownership).

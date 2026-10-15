# Advanced element mechanisms

**Contents:** [Custom measurement](#custom-measurement) · [Async and caches](#async-and-caches) · [Virtualization](#virtualization)

## Custom measurement

For custom layouts, distinguish requesting nodes from resolving geometry.
A new child's LayoutId has no usable final bounds during request_layout.
Use request_measured_layout when size depends on available space, or an
established custom layout implementation that computes layout at the proper
phase before reading bounds. Inspect the [locked Window layout APIs](https://docs.rs/crate/gpui-pre/0.3.9/source/src/window.rs)
and [Element lifecycle](element.md).

Keep child instances and state through all phases, and position/prepaint them
using the actual AnyElement APIs. Do not pass a shared parent ID manually as
though it gave every child independent identity. Use explicit namespaces and
stable child IDs when retaining per-child behavior.

## Async and caches

Start asynchronous work in the retained owner from an event or lifecycle
method. Store loading/result/error state there and render the current snapshot;
duplicating those fields in a transient Element does not synchronize them.
Capture a weak owner in long-running work, retain a cancellable Task where
appropriate, and reject stale completions. See [async contracts](async.md).

A `'static` task cannot capture `&mut self` from paint or a borrowed update.
Likewise, a cache field on an element recreated each render is not a persistent
cache. Put persistent caches in an Entity/keyed state, with explicit keys for
content, available width, typography, rem size, and other measured inputs.

## Virtualization

Use existing list/table virtualization unless the behavior requires a new
implementation. Visible-index arithmetic alone omits variable heights,
scroll-to-item, keyboard focus, clipping, and invalidation after zoom/content
changes. Keep domain identity separate from positional layout coordinates.
See the normative [large-data guide](../coding-guides.md#lists-tables-and-large-data)
and [review checklist](element-best-practices.md). These are mechanism notes,
not complete masonry, circular-layout, or virtual-list implementations.

# Entity state management

**Contents:** [Ownership](#ownership) · [Access](#access) · [Re-entrancy](#re-entrancy) · [References](#references)

## Ownership

Entity<T> is a strong handle to retained state; cloning it shares that state.
WeakEntity<T> does not keep the state alive. Create retained state once in the
owning constructor and keep a strong handle in the owner. A parent normally
owns its children strongly; a child's back-reference should not create a
strong ownership cycle. Making every relationship weak can instead release
state while the UI still needs it.

A Render method builds a fresh element description from those entities.
Input/Select state and subscriptions belong to the owning view, not a local
variable recreated and dropped by render. Follow the normative
[state ownership guide](../coding-guides.md#state-ownership).

## Access

For `gpui-pre` 0.3.9, used by the locked `gpui-kit` 0.8.0:

| Operation | Result |
| --- | --- |
| `Entity::read(&App)` | Borrowed `&T` |
| `Entity::read_with(cx, closure)` | Closure result `R` |
| `Entity::update(cx, closure)` | Closure result `R` |
| `WeakEntity::upgrade()` | `Option<Entity<T>>` |
| `WeakEntity::read_with` / `update` | `Result<R>` |

These updates are synchronous, including when called from AsyncApp. Await only
a future/Task returned by the closure, not the update itself. Use the inner
context supplied by the update, and call notify after a coherent mutation
that changes the rendered state. See the [versioned entity source](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/entity_map.rs).

## Re-entrancy

An entity cannot be read or updated through its handle while that same entity
is mutably borrowed by an update/render/listener. Use the direct `&mut T`
provided to the callback. Accessing a different entity is allowed, but a
callback cycle back into the first entity still panics.

Context::defer_in borrows the owning entity again when the deferred closure
runs. It does not bypass this rule. In a list delegate callback, use the list
state already supplied; do not reach back to it via a parent entity that owns
that same list. See the [defer_in implementation](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/context.rs).

Render may read other entities when the ownership/borrow path permits it.
When a delegate callback would read an owner that is already borrowed, keep a
plain presentation snapshot and refresh it through the mutation/event path
before rendering. Do not create a blanket ban on all external entity reads or
all nested updates; trace the actual cycle.

## References

- [API details](entity-api.md): weak operations, window access, lifecycle.
- [Patterns](entity-patterns.md): model/view, observations and owner callbacks.
- [Review checklist](entity-best-practices.md): cycles, subscriptions and notification.
- [Advanced state](entity-advanced.md): collections, debounce, snapshots and transitions.
- [Testing](test-reference.md): real interaction and asynchronous outcomes.

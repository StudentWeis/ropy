# Entity API contracts

**Contents:** [Handle operations](#handle-operations) · [Entity context](#entity-context) · [Lifecycle](#lifecycle)

## Handle operations

Use the [gpui-pre 0.3.9 entity source](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/entity_map.rs)
for exact bounds and return types. This is the core selected by the repository's
`gpui-kit` 0.8.0 dependency.

Strong `Entity<T>::update` takes an AppContext and
`FnOnce(&mut T, &mut Context<T>) -> R`, returning R directly. `read_with` takes
`FnOnce(&T, &App) -> R`. Neither operation is asynchronous. A read through
`Entity::read` borrows from an App; use read_with with other AppContext types.

WeakEntity::upgrade returns Option, while weak read_with/update return Result
when the entity has been released. Do not assume every weak operation returns
Result. Strong update_in accepts a VisualContext and returns its associated
result type; weak update_in returns Result and resolves the entity's current
window. Its callback receives `(&mut T, &mut Window, &mut Context<T>)`.
Handle a missing window or released entity rather than assuming a task keeps
both alive.

## Entity context

Context::entity and weak_entity expose handles for the current owner.
Context::listener binds an element callback to that owner; observe and subscribe
return Subscriptions. The [Context source](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/context.rs)
and [context table](context.md#callbacks) specify callback arity.

EntityId identifies an entity without borrowing its contents. AnyEntity erases
the concrete type; use its supported downcast operations instead of inventing
`Entity<dyn Trait>` registries. Prefer a typed owner/model boundary where the
application does not need type erasure.

## Lifecycle

The final strong handle's removal permits entity release. A weak handle does
not preserve it. Keep intended ownership explicit, avoid strong reference
cycles, and retain subscriptions/tasks when their lifetime should follow the
owner. Detaching a subscription or task is a deliberate lifetime choice, not
a universal cleanup rule.

Use [entity ownership and re-entrancy](entity.md) before designing references,
and [async contracts](async.md) before putting entity access in tasks. These
notes are not a parallel exhaustive API catalog; the versioned source is the
authority when signatures change.

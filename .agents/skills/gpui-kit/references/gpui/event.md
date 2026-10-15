# Events and subscriptions

**Contents:** [Choose the signal](#choose-the-signal) · [Callback contracts](#callback-contracts) · [Lifetime and ordering](#lifetime-and-ordering)

## Choose the signal

Use Context::notify to invalidate an entity and inform its observers. Use
Context::emit for a semantic event, with `impl EventEmitter<Event> for Owner {}`
on the emitting type. Deriving Clone on the event does not supply that trait.
An observer rereads state; an event subscriber receives the typed event.

These are the locked `gpui-pre` 0.3.9 contracts used by `gpui-kit` 0.7.3.
See [Context::emit, observe, and subscribe](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/context.rs).
For window commands, use [Actions](action.md) instead of inventing an event bus.

## Callback contracts

From an owning `Context<T>`:

| API | Callback parameters |
| --- | --- |
| `observe(&entity, ...)` | `&mut T, Entity<Observed>, &mut Context<T>` |
| `subscribe(&entity, ...)` | `&mut T, Entity<Emitter>, &Event, &mut Context<T>` |
| `subscribe_in(&entity, window, ...)` | `&mut T, Entity<Emitter>, &Event, &mut Window, &mut Context<T>` |
| `subscribe_self(...)` | `&mut T, &Event, &mut Context<T>` |
| `observe_global::<G>(...)` | `&mut T, &mut Context<T>` |
| `observe_window_activation(window, ...)` | `&mut T, &mut Window, &mut Context<T>` |

App-level callbacks have different arity; in particular App::observe_global
receives only `&mut App`. Do not copy it into an entity method unchanged.
Use `window.is_window_active()` for activation and [focus subscriptions](focus-handle.md)
for an individual focus target.

## Lifetime and ordering

Each of these registrations returns a Subscription. Store it on the owner
when cancellation should follow that owner; dropping a local registration
immediately unregisters it. Detach intentionally when the registration should
outlive that handle. Context subscriptions already use a weak subscriber;
that does not mean every captured strong handle is harmless or every
subscription should be detached. See the [Subscription implementation](https://docs.rs/crate/gpui-pre/0.3.9/source/src/subscription.rs).

Register in construction/lifecycle code rather than every render. Keep one
source of truth, avoid mutual event/notification loops, and check
[re-entrancy](entity.md#re-entrancy) through owner/delegate callbacks. An emitted
event is processed through GPUI's effects; do not assume it is an immediate
synchronous call or that it automatically redraws the emitter.

Use typed entities and events rather than an unsupported `WeakEntity<dyn Trait>`
collection. Test the actual producer/consumer path, retain the registration,
and assert the outcome and event count after the appropriate update/frame.

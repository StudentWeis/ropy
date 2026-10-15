# Context management

**Contents:** [Choose the context](#choose-the-context) · [Callbacks](#callbacks) · [Deferred work](#deferred-work)

## Choose the context

The following describes the locked `gpui-kit` 0.7.3 / `gpui-pre` 0.3.9 APIs.
Contexts are capabilities supplied for an operation, not an inheritance tree.

| Context | Responsibility |
| --- | --- |
| `App` | Application state, globals, entity creation and access |
| `Context<T>` | App access plus the current entity, listeners, events, notification |
| `Window` | Focus, input dispatch, window geometry and rendering phases |
| `AsyncApp` | Application access from foreground async tasks |
| `AsyncWindowContext` | Async application access associated with a window |

Retain typed handles such as `Entity`, `WeakEntity`, or `FocusHandle`; do not
retain the borrowed `&mut` contexts. `Window::is_window_active()` checks native
window activation; `FocusHandle::is_focused(window)` checks a focus target.
They describe different state.

## Callbacks

Callback arity depends on the receiver. For `Context<T>`:

| Operation | Callback parameters |
| --- | --- |
| `listener` | `&mut T, &Event, &mut Window, &mut Context<T>` |
| `observe` | `&mut T, Entity<Observed>, &mut Context<T>` |
| `subscribe` | `&mut T, Entity<Emitter>, &Event, &mut Context<T>` |
| `subscribe_in` | `&mut T, Entity<Emitter>, &Event, &mut Window, &mut Context<T>` |
| `observe_global` | `&mut T, &mut Context<T>` |
| `observe_window_activation` | `&mut T, &mut Window, &mut Context<T>` |
| `spawn` | `WeakEntity<T>, &mut AsyncApp` |
| `spawn_in` | `WeakEntity<T>, &mut AsyncWindowContext` |

In particular, `Context<T>::spawn` takes a two-argument async closure, while
`App::spawn` takes one. `Context<T>::observe_global` takes two arguments, while
`App::observe_global` takes one. Keep the returned Subscription alive when
observing/subscribing. Use the context provided to the callback rather than
capturing the outer borrow.

The [locked Context implementation](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/context.rs)
and [App implementation](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app.rs)
are the signature references. See [events](event.md), [async tasks](async.md),
and [focus](focus-handle.md) for their lifetime contracts.

## Deferred work

`App::defer` runs a closure with `&mut App` after the current update.
`Context<T>::defer_in(window, ...)` runs with
`(&mut T, &mut Window, &mut Context<T>)`. The latter re-borrows the owning
entity when invoked. Operate on its supplied `&mut T`; reading or updating
that same entity through its handle re-enters the borrow and panics. A callback
through a different entity can also cycle back, so inspect the entire path.

An entity's `cx.notify()` schedules observers/redraw after a state change; it
is not an event payload or a synchronous render call. Keep render declarative.
For complete initialization and Root ownership, use the
[application recipes](../recipes.md), and follow the normative
[phase and context guide](../coding-guides.md#understand-gpuis-phases-and-contexts).

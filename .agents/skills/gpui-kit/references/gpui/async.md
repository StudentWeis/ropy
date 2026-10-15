# Async work and task lifetime

**Contents:** [Spawn contexts](#spawn-contexts) · [Updating entities](#updating-entities) · [Cancellation and ordering](#cancellation-and-ordering)

## Spawn contexts

These contracts are from `gpui-pre` 0.3.9, used by this repository's
`gpui-kit` 0.7.3. The spelling `cx.spawn` alone does not identify its callback:

| Receiver | Async closure parameters |
| --- | --- |
| `App` | `&mut AsyncApp` |
| `Context<T>` | `WeakEntity<T>, &mut AsyncApp` |
| `Context<T>::spawn_in(window, ...)` | `WeakEntity<T>, &mut AsyncWindowContext` |
| `Window::spawn(cx, ...)` | `&mut AsyncWindowContext` |

See [Context spawn and spawn_in](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/context.rs),
[App spawn](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app.rs), and
[Window spawn](https://docs.rs/crate/gpui-pre/0.3.9/source/src/window.rs).
These foreground tasks may await work; their entity mutations remain on the
application thread. `background_spawn` takes an owned `Send + 'static` future.
Move computation/I/O inputs into it, then apply its result from the foreground.
Do not move a borrowed `Window`, `App`, or `Context` into a task.

## Updating entities

`WeakEntity::update` and `update_in` are synchronous and return `Result<R>`.
They do not become futures inside an async closure. Await the I/O or Task,
then perform the update. This fragment belongs inside an entity method with
an application-defined `load_data` function and `data` field:

```rust
cx.spawn(async move |this, cx| {
    let data = load_data().await;
    if this.update(cx, |state, cx| {
        state.data = data;
        cx.notify();
    }).is_err() {
        return; // The owner was released.
    }
}).detach();
```

If the result needs a window, use `spawn_in` and `update_in`, handling a closed
window or released entity. A strong `Entity::update` returns the closure's `R`
directly; do not add `.ok()` or `.await` unless that closure itself returns a
Result or future. See the [entity implementations](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/entity_map.rs).

## Cancellation and ordering

A Task is cancelled when dropped. Retain it in the owning entity when replacing
or dropping that owner should cancel the work; detach only when that lifetime
is intentional. For periodic work, stop after a weak update fails instead of
leaving a detached loop alive after the view closes. Await background Tasks
inside the foreground task; there is no `Task::then` continuation API here.

Start work from a command or lifecycle method, not unconditionally from render.
Use request identity/revisions to reject stale results. Debouncing needs retained
pending state and a replaceable Task, not a closure capturing `&mut self`.
Use GPUI executor timers unless the application has deliberately installed
another runtime; a Tokio timer does not acquire a runtime merely because GPUI
is running.

`defer_in` is a deferred entity update, not an escape from borrowing rules: its
callback already holds that entity mutably. See [re-entrancy](entity.md#re-entrancy)
and [test scheduling](test-reference.md#async-and-timers). The normative
[async guide](../coding-guides.md#async-work-and-side-effects) covers loading,
error, and stale-result presentation.

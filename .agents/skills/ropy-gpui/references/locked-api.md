# Locked API notes

These notes preserve Ropy's verified corrections for `gpui-kit` 0.8.0 and its default `gpui-pre` 0.3.9 engine. Read the installed source before using an upstream example with a conflicting signature. This is a compatibility note, not another framework manual.

## Contexts and async work

The [Context source](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/context.rs) defines these callbacks:

| Receiver and operation | Callback arguments |
| --- | --- |
| `Context<T>::listener` | `&mut T, &Event, &mut Window, &mut Context<T>` |
| `Context<T>::observe` | `&mut T, Entity<Observed>, &mut Context<T>` |
| `Context<T>::subscribe` | `&mut T, Entity<Emitter>, &Event, &mut Context<T>` |
| `Context<T>::subscribe_in` | `&mut T, &Entity<Emitter>, &Event, &mut Window, &mut Context<T>` |
| `Context<T>::observe_global` | `&mut T, &mut Context<T>` |
| `Context<T>::spawn` | `WeakEntity<T>, &mut AsyncApp` |
| `Context<T>::spawn_in` | `WeakEntity<T>, &mut AsyncWindowContext` |
| `App::spawn` | `&mut AsyncApp` |
| `Window::spawn` | `&mut AsyncWindowContext` |

`Entity::update` returns the closure result directly. `WeakEntity::update` and `update_in` return a synchronous Result; do not await them. `WeakEntity::upgrade` returns Option. Await I/O or background Tasks, then update the owner. There is no `Task::then` continuation API in this engine. Retain cancellable Tasks and Subscriptions on their owner; detach only for an intentional independent lifetime.

`Context::defer_in` already borrows the owner mutably. Use the supplied `&mut T`; accessing the same entity through another handle re-enters its borrow. See the [entity source](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/entity_map.rs).

## Global state and focus

`set_global`, `global_mut` and completion of `update_global` queue observer notifications; manually broadcasting a second notification is unnecessary. Mutating data behind a shared lock does not invoke those hooks. `Global` does not require Clone. Use `Theme::update` to synchronize theme colors, renderable tokens and the Base projection.

Focus queries take a Window: `handle.is_focused(window)`. Native activation is `window.is_window_active()`. Create retained handles with `cx.focus_handle().tab_stop(true)` when they belong in Tab order; `track_focus` does not add a tab stop by itself. Focus subscriptions (`on_focus`, `on_blur`, `on_focus_in`, `on_focus_out`) are Context methods, not div builders. Bind keys before setting native menus. See the [Window source](https://docs.rs/crate/gpui-pre/0.3.9/source/src/window.rs).

## Elements and component constructors

`Element` has two associated states, `RequestLayoutState` and `PrepaintState`; its trait methods differ from type-erased `AnyElement` helpers. `AnyElement::request_layout(window, cx)` returns a LayoutId, `prepaint(window, cx)` returns an optional FocusHandle, and `paint(window, cx)` returns unit. Do not pass Element trait IDs, bounds or state arguments to those wrapper helpers. See the [Element source](https://docs.rs/crate/gpui-pre/0.3.9/source/src/element.rs).

`Window::with_element_state` receives a GlobalElementId and a closure mapping `Option<S>` to `(result, S)`. Element fields do not establish persistent application state. Register hitboxes in prepaint and input handlers in paint; the latter capture owned data or handles, not a temporary `&mut self`.

`Input::new` and `Select::new` receive retained state entities, not string IDs. `Tab::new()` takes no ID argument. `h_flex()` centers on the cross axis; `v_flex()` retains stretch behavior. Full-height columns inside rows may need explicit height or stretch. Verify constructors in the [0.8.0 component source](https://github.com/longbridge/gpui-kit/tree/1676826d9d49b90f4d7b6808b69924f58796f6f1/crates/component/src).

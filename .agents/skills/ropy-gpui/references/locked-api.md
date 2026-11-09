# Locked API notes

These notes preserve Ropy's verified corrections for `gpui-kit` 0.9.1 and its default `gpui-pre` 0.3.12 engine. Read the installed source before using an upstream example with a conflicting signature. This is a compatibility note, not another framework manual.

## Keyboard and scroll APIs

`KeyBinding::new` and `Keystroke::parse` require static strings with explicit modifiers. Use `KeyBinding::load(..., cx)` for configuration strings and `Keystroke::parse_with_os(source, cx.platform_info().os)` for dynamic parsing. `Keystroke::unparse` takes the host operating system; do not infer it from the compilation target. The `secondary` modifier resolves from that runtime host. `InteractiveElement::on_scroll` replaces the deprecated `on_scroll_wheel`.

## Contexts and async work

The [Context source](https://docs.rs/crate/gpui-pre/0.3.12/source/src/app/context.rs) defines these callbacks:

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

### Entity borrow chains

`Context::defer_in` already borrows the owner mutably. Use the supplied `&mut T`; accessing the same entity through another handle re-enters its borrow. The prohibition applies to entities already mutably leased in the current render/update/callback chain, not to every external `Entity::read`. Reading another entity that is not in that borrow chain is valid; a cycle such as A updating B and B reading or updating A is not. Ordinary shared reads alone do not create a mutable lease. The [entity source](https://docs.rs/crate/gpui-pre/0.3.12/source/src/app/entity_map.rs) removes an entity from the map while leased, which is why reading or updating that same entity panics.

The upstream [entity best practices](../../gpui-kit/references/gpui/entity-best-practices.md) overstate the restriction on external reads in rendering hooks. Capture a snapshot before entering a callback when it would otherwise reach an entity in the active borrow chain; do not clone all external state merely to avoid legal reads.

### Transaction error paths

The upstream [Entity Transaction Pattern](../../gpui-kit/references/gpui/entity-advanced.md#entity-transaction-pattern) is an illustrative helper, not a GPUI transaction API. Its `Drop` implementation only prints a warning; it does not restore `original_state`. In its usage example, `validate_state(&tx.entity, cx)?` can return early after `tx.update` has already mutated the live entity, leaving those mutations in place. The explicit `rollback(cx)` handles `Ok(false)` only, not `Err` or unwinding.

Do not rely on Drop for rollback. Prefer preparing and validating an independent candidate before updating the live entity once. A candidate must not share mutable backing state with the live entity; a shallow Clone of handles is not isolation. If adapting the helper, explicitly handle both validation rejection and error, call `rollback(cx)` before returning either failure, and reserve `commit(cx)` for success. This still does not reverse already emitted events, I/O or mutations in other entities, nor guarantee recovery from a panic. Keep fallible work and external effects outside the live-state mutation boundary unless an explicit recovery contract covers them.

## Global state and focus

`set_global`, `global_mut` and completion of `update_global` queue observer notifications; manually broadcasting a second notification is unnecessary. Mutating data behind a shared lock does not invoke those hooks. `Global` does not require Clone. Use `Theme::update` to synchronize theme colors, renderable tokens and the Base projection.

Focus queries take a Window: `handle.is_focused(window)`. Native activation is `window.is_window_active()`. Create retained handles with `cx.focus_handle().tab_stop(true)` when they belong in Tab order; `track_focus` does not add a tab stop by itself. Focus subscriptions (`on_focus`, `on_blur`, `on_focus_in`, `on_focus_out`) are Context methods, not div builders. Bind keys before setting native menus. See the [Window source](https://docs.rs/crate/gpui-pre/0.3.12/source/src/window.rs).

## Elements and component constructors

`Element` has two associated states, `RequestLayoutState` and `PrepaintState`; its trait methods differ from type-erased `AnyElement` helpers. `AnyElement::request_layout(window, cx)` returns a LayoutId, `prepaint(window, cx)` returns an optional FocusHandle, and `paint(window, cx)` returns unit. Do not pass Element trait IDs, bounds or state arguments to those wrapper helpers. See the [Element source](https://docs.rs/crate/gpui-pre/0.3.12/source/src/element.rs).

`Window::with_element_state` receives a GlobalElementId and a closure mapping `Option<S>` to `(result, S)`. Element fields do not establish persistent application state. Register hitboxes in prepaint and input handlers in paint; the latter capture owned data or handles, not a temporary `&mut self`.

`Input::new` and `Select::new` receive retained state entities, not string IDs. `Tab::new()` takes no ID argument. `h_flex()` centers on the cross axis; `v_flex()` retains stretch behavior. Full-height columns inside rows may need explicit height or stretch. Verify constructors in the [0.9.1 component source](https://github.com/longbridge/gpui-kit/tree/39ab849a93b5f15507b969164d49891db8d5b470/crates/component/src).

### Masonry child mapping

Do not copy the upstream [Masonry Layout example](../../gpui-kit/references/gpui/element-advanced.md#masonry-layout-pinterest-style) as a working implementation. Each column stores only LayoutIds, then enumerates its children starting at zero in prepaint and indexes `self.children[child_idx]` with that column-local index. Multiple columns therefore revisit the wrong children. It also appends bounds in column traversal order and later zips them with children in original order, mismatching the paint mapping.

Retain the original child index alongside each LayoutId when assigning columns, for example as `(child_ix, layout_id)`. Carry that same index with each resolved bound through prepaint and paint; use the same traversal order for both phases and address the original child by that index. Do not recover identity from a column-local enumeration or zip column-ordered bounds with the original children. A regression test should use multiple columns and unequal child heights and verify that every child is prepainted and painted exactly once with its own placement. The example also uses outdated `AnyElement` lifecycle signatures and reads child bounds before layout resolution; the index correction alone does not make it runnable. Check the installed Element implementation and resolve measurement in the appropriate layout phase.

## 0.9.1 release-note API gaps

The v0.9.1 tagged source and published `gpui-component` 0.9.1 source do not expose `Button::selected_accent` or themed `Slider::track_focus`, despite the release notes advertising them. Only the unstyled Base slider exposes `track_focus(&FocusHandle)`. Ropy uses the themed component; do not assume a Base builder is forwarded or replace the component to bypass the missing API. Keep existing theme selection styling and slider focus behavior until an installed version supplies the intended API.

## Animation timing and tests

For `gpui-kit` 0.9.1 with `gpui-pre` 0.3.12, both non-synced and synced `Animation` use `cx.background_executor().now()`; spring animations use that executor clock too. See the installed [animation source](https://docs.rs/crate/gpui-pre/0.3.12/source/src/elements/animation.rs). The wall-clock statement in the upstream [testing frame/async guidance](../../gpui-kit/references/gpui/test.md#frames-and-async-work) does not apply to this lockfile.

In headless animation tests, first render to establish element-keyed animation state, then advance the test executor clock and draw/process the next frame before observing the animated result. Keep the element ID stable across those frames and explicitly set the intended reduced-motion policy. Use `VisualTestContext::advance_clock(duration)` or `TestAppContext::executor().advance_clock(duration)`; do not use wall-clock sleeps to progress these animations. Executor progress without a rendered frame is not evidence of a changed visual result. See also the [0.9 integration notes](integration.md).

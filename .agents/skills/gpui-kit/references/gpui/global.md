# Global state

**Contents:** [Ownership](#ownership) · [Mutation and observation](#mutation-and-observation) · [Services and async work](#services-and-async-work)

## Ownership

A type implementing Global can have one application-wide value stored in App.
Use this for application configuration and shared services; keep feature/view
state with its narrowest owner. Global does not mean thread-safe or immutable,
and Clone is not required by the Global trait.

The [Global trait](https://docs.rs/crate/gpui-pre/0.3.9/source/src/global.rs),
[App storage](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app.rs), and
[BorrowAppContext operations](https://docs.rs/crate/gpui-pre/0.3.9/source/src/gpui.rs)
are fixed to gpui-pre 0.3.9, used by the repository's gpui-kit 0.7.3 dependency.

## Mutation and observation

Set a value with set_global, read it with global, and mutate through
update_global/global_mut. Register the global before a required read/update;
missing required globals panic. Avoid re-entering the same global from inside
its update closure, where it is temporarily leased out of App storage.

In this version, set_global and global_mut queue global-observer notification;
update_global restores its lease through end_global_lease, which also queues
notification. They do **not** require manually broadcasting a separate event
to all global observers. A view that depends on a global should retain an
observer and notify itself when it needs a redraw.

For `Context<T>`, the observer shape is
`cx.observe_global::<Settings>(|this, cx| ...)`; for `App`, it is
`cx.observe_global::<Settings>(|cx| ...)`. Store the returned Subscription.
Mutating data behind an Arc/lock/atomic obtained from a shared reference does
not pass through GPUI's global mutation hooks; define an explicit notification
path when that data affects the UI.

For component theme changes, use Theme::update and follow the normative
[theme guide](../coding-guides.md#theme-and-styling): ordinary global observer
notification does not by itself synchronize the theme's derived tokens and
Base projection.

## Services and async work

Clone an owned service handle out of the global before starting async work;
do not hold an App/global borrow across await. Context::spawn receives
`(WeakEntity<T>, &mut AsyncApp)`, while App::spawn receives only AsyncApp.
Keep UI mutation on the foreground context and handle owner release/stale
results. See [async contracts](async.md) and [event lifetime](event.md).

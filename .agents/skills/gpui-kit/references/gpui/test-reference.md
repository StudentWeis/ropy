# Testing entity and async contracts

**Contents:** [Choose the assertion boundary](#choose-the-assertion-boundary) · [Async and timers](#async-and-timers) · [Re-entrancy and deferred selection](#re-entrancy-and-deferred-selection)

## Choose the assertion boundary

Use [the UI integration workflow](test.md) for dependency setup, headless
windows, real pointer/keyboard dispatch, and native observations. Pure model
logic belongs in ordinary Rust tests; entity lifecycle and subscriptions can
use TestAppContext without a visible workflow.

Retain subscriptions when testing events. The source must implement
EventEmitter<Event>, and the test must drive its actual emission path rather
than duplicating the mutation. For keyboard actions, establish the intended
focus path and dispatch the actual key/Action; direct handler calls do not
validate bindings or focus.

## Async and timers

Entity::update is synchronous and returns the closure's R. WeakEntity::update
is also synchronous and returns Result<R>. Inside a spawned async task,
`this.update(cx, |state, cx| ...)` returns a Result, not a future. If its
closure returns a Task, first extract that Task from the successful Result.
Handle the error for a released weak owner.

If an entity method returns Task<R>, extract that Task with update and then
await the Task. This contextual fragment assumes the application's load method
returns a Task:

```rust
let task = component.update(cx, |state, cx| state.load(cx));
let result = task.await;
```

Use the locked [entity source](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/entity_map.rs),
[TestAppContext](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/test_context.rs),
and [executor](https://docs.rs/crate/gpui-pre/0.3.9/source/src/executor.rs)
for scheduling behavior. In gpui-pre 0.3.9, run_until_parked may advance to
pending timers while draining work. For a timing-boundary test, use the test
executor's explicit clock/tick controls and assert before and after the
boundary; do not equate one drain with one frame or one timer tick. A repeating
task needs a bounded stop/cancellation condition in the test.

For deferred UI results, leave the current window update and wait for an
observable condition with the existing wait helpers. Avoid wall-clock sleeps
or arbitrary millisecond performance assertions. Enable external-I/O parking
only when the test actually relies on work outside the deterministic scheduler.

## Re-entrancy and deferred selection

Exercise the real component through pointer and keyboard input. A successful
construction test or a should_panic test does not show that the interaction
avoids a borrow cycle. Assert the committed model value and visible outcome
after the deferred work, plus an appropriate negative path.

The [locked Select implementation](https://github.com/longbridge/gpui-kit/blob/1676826d9d49b90f4d7b6808b69924f58796f6f1/crates/component/src/select.rs)
commits through defer_in while its list state is borrowed. Its on_will_change
hook edits the proposed selection directly; effects needing a GPUI context
belong in the subsequent owner/confirmation path. Do not access the same list
through an entity handle from that callback.

Cover confirmation, cancellation, veto/edited selection when supported,
repeated interactions, and agreement between selected_value and the displayed
trigger. Keep state owned by the production view and use its public behavior;
do not expose private list internals solely for the test. Follow the
[entity borrow rules](entity.md#re-entrancy) when writing recording delegates.

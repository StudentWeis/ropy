# Advanced entity state

**Contents:** [Collections](#collections) · [Debounce and asynchronous state](#debounce-and-asynchronous-state) · [Snapshots and transitions](#snapshots-and-transitions)

## Collections

An owning collection holds strong Entity handles; an index of independently
owned objects may hold WeakEntity handles and remove entries that no longer
upgrade. Domain IDs, EntityIds, and current list indexes have different roles.
Do not use a position as persistent UI identity. Add a registry only when
multiple owners need that lookup contract, and prefer typed handles.

## Debounce and asynchronous state

Store the pending value and replaceable Task in a retained owner. Each new
request replaces/cancels the previous task or advances a revision checked on
completion. A task cannot capture a method's borrowed `&mut self` in its
`'static` closure. Capture the weak entity supplied by Context::spawn and use
synchronous update after awaiting the timer/work. See [async contracts](async.md).

Distinguish debounce (wait for quiet) from throttle (limit rate); an elapsed-time
check alone does not provide both. Define whether the first or last value wins,
and test cancellation, release, and out-of-order completion. Use GPUI timers
with GPUI's test scheduler rather than assuming a Tokio runtime.

## Snapshots and transitions

A presentation snapshot is useful when a delegate callback cannot safely read
its already-borrowed owner. Refresh it on the normal mutation/event path and
include revision/identity when results can arrive later. It must not become an
independent source of truth that drifts from the model.

Represent loading, ready, and failure transitions explicitly and update fields
that form an invariant together. Sequential updates to several entities are
not a transaction or rollback mechanism: observers and callbacks still have
their own timing. Put an invariant in one owner or define an explicit domain
operation; do not promise atomicity through a generic example wrapper.

Pooling, proxies, and cascading observers introduce lifecycle and re-entrancy
costs. Use them only for a demonstrated need and follow the
[state ownership guide](../coding-guides.md#state-ownership),
[entity borrow rules](entity.md#re-entrancy), and [test guidance](test-reference.md).

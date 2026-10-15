# Reviewing entity state

**Contents:** [Ownership and borrowing](#ownership-and-borrowing) · [Notifications and lifetime](#notifications-and-lifetime) · [Validation](#validation)

## Ownership and borrowing

Follow the normative [state ownership guide](../coding-guides.md#state-ownership)
and the [locked entity contracts](entity-api.md). Check that every retained
state has an intended strong owner, weak back-references break actual cycles,
and callbacks do not capture a borrowed `&mut self` or context.

Trace the entire callback path before nesting access. Reading/updating the
same entity while its mutable borrow is active panics; accessing a different
entity is permitted unless it calls back into the first. Defer_in still
borrows its owner. Use supplied direct references and snapshots where needed,
without forbidding ordinary reads of independent model entities.

## Notifications and lifetime

Notify after a meaningful, coherent mutation. Reading data or rendering is not
itself a reason to notify. Avoid observer feedback loops and redundant
notifications. A semantic event and an invalidation notification have different
purposes; emitting one does not automatically substitute for the other.

Store Subscriptions when the owner should cancel them. Detach deliberately,
not as a blanket best practice. Likewise, retain replaceable Task handles and
stop weak-owner loops after release. Strong handles in closures are legitimate
when they intentionally own the resource and cannot form a cycle.

Reuse state entities across frames. Cache only measured expensive work and
make invalidation explicit. Do not introduce pooling, transactions, or registries
without an application behavior that requires them.

## Validation

Use pure tests for domain transitions, GPUI context tests for entity/event
behavior, and real UI interaction tests for focus, callback ordering, redraw,
and re-entrancy. Assert semantic outcomes and event counts; a test that only
constructs state or directly calls an internal callback does not validate the
interaction. See [testing](test.md) and [test contracts](test-reference.md).

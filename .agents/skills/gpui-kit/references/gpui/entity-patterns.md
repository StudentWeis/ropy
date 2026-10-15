# Entity ownership patterns

**Contents:** [Model and view](#model-and-view) · [Events and observations](#events-and-observations) · [Tasks and resources](#tasks-and-resources)

## Model and view

Keep pure domain state as ordinary Rust data unless retained identity,
observation, or GPUI access is needed. A view can strongly own a model entity
and child state entities; choose weak back-references where a strong cycle
would otherwise occur. Do not create an entity for every presentational fragment.
See the normative [choice of unit](../coding-guides.md#choose-the-right-unit).

Controlled values have one owner. A component receives the current value and
reports requested changes; its owner updates and notifies. Retained components
such as Input instead keep their behavioral state in a retained Entity. Create
that state in a constructor and pass its reference to the rendered component.

## Events and observations

Use notify/observe when observers need to reread state, and typed emit/subscribe
for semantic events. The emitter implements EventEmitter<Event>. Retain the
returned Subscription, or detach intentionally when explicit cancellation is
not needed. Register once in construction/lifecycle code, not on every render.
See [event contracts](event.md).

A callback may update a different entity, but must not cycle back into an
already borrowed owner. Flatten the updates, emit an owner event, or supply a
presentation snapshot where a rendering delegate cannot safely read its owner.
The [re-entrancy rules](entity.md#re-entrancy) apply to deferred callbacks too.

## Tasks and resources

Keep Task handles in the owner when work should cancel on replacement/drop.
Weak handles prevent a background operation from retaining a closed view; a
released owner is a normal completion case. Reject stale results with request
identity/revisions and notify once after applying a coherent result.

Subscriptions, task handles, and external resources need a clear lifetime.
Prefer their existing RAII behavior over a generic registry or manager whose
only job is forwarding calls. Use [async contracts](async.md) and the maintained
[application recipes](../recipes.md) for actual API usage.

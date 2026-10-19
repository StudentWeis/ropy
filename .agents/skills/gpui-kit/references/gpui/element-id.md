# Element identity

Follow [Stable identity](../coding-guides.md#stable-identity). Ropy locks
`gpui-kit` 0.8.0, with `gpui-pre` 0.3.9 as its default engine. See the
[versioned ElementId definition and conversions](https://docs.rs/crate/gpui-pre/0.3.9/source/src/window.rs).

An `ElementId` identifies a node within its parent's identity scope. GPUI
combines ancestor IDs into a `GlobalElementId`; sibling IDs must be distinct.
A stable ID lets element-local state survive reconstruction of the element
tree. Changing an ID deliberately resets that identity.

Use a fixed string for a single named control and a domain-derived ID for
repeated controls. Never key reorderable or filterable rows by their current
index, translated label, or a random value created during render. A position
passed to a selection API is not a persistent identity.

These are builder fragments inside a renderer, with `project.id: u64`:

```rust
Button::new("save").label("Save")
Button::new(("delete-project", project.id)).label("Delete")
div().id(("project-row", project.id))
```

Import `Button` from `gpui_kit::component::button` and GPUI's element traits
where needed. String literals (`&'static str`), owned `String`, `SharedString`,
integer IDs and supported namespace tuples convert into `ElementId`; an
arbitrary borrowed `&str` is not the same as a static string literal.

## Identity and retained state are separate

For a `div`, `.id(...)` creates a `Stateful<Div>` for keyed state and APIs
such as `on_click`. Do not generalize this to all mouse events: lower-level
mouse handlers are also available through `InteractiveElement`.

Not every component constructor takes an ID. Stateful controls receive the
entity retained by their owning view; their default identity derives from
that entity. An explicit `.id(...)` customizes the frame identity without
replacing the state:

```rust
Input::new(&self.search_input).id("search-input")
Select::new(&self.country_select).id("country-select")
```

These are render fragments, not standalone programs. See the verified 0.8.0
[Input constructor](https://github.com/longbridge/gpui-kit/blob/1676826d9d49b90f4d7b6808b69924f58796f6f1/crates/component/src/input/input.rs) and
[Select constructor](https://github.com/longbridge/gpui-kit/blob/1676826d9d49b90f4d7b6808b69924f58796f6f1/crates/component/src/select.rs), and
[component conventions](../conventions.md) for state ownership. Allocate the
state once in the owning view's constructor, not on every render.

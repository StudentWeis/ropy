# GPUI Kit usage in Ropy

This reference targets the locked `gpui-kit` 0.8.0 API. See
[component conventions](conventions.md), the normative [Coding Guides](coding-guides.md),
and the [pinned upstream recipes](recipes.md) for complete source context.
The snippets below are fragments for an existing view, not standalone programs.
Use Ropy's localization keys for application labels and placeholders.

## Setup and window ownership

Keep the dependency features already selected in Ropy's `Cargo.toml` and
`Cargo.lock`. Do not add another GPUI dependency or switch engines to make an
example compile. Initialize with `gpui_kit::init(cx)` before constructing
component-backed views. `gpui_kit::open_window(options, cx, |window, cx| ...)`
returns `(AnyWindowHandle, Entity<V>)` and installs a `Root` around the view.
That root owns dialogs, sheets and notifications; application renderers do
not mount another set of overlay layers.

For a full application entry point, use the
[0.8.0 upstream bootstrap](https://github.com/longbridge/gpui-kit/blob/1676826d9d49b90f4d7b6808b69924f58796f6f1/examples/ai_recipes/src/bootstrap.rs).
The current [Getting Started](https://gpui-kit.com/docs/getting-started.md)
guide explains new-project setup. Upstream examples and verification commands
are not files or commands in Ropy; see [recipes](recipes.md) for that boundary.

## Retained controls

Allocate state in the owning view's constructor and store the entity as a
field. Rendering passes a reference to that same entity:

```rust
use gpui_kit::component::input::{Input, InputState};

// Constructor: retain this as self.search_input: Entity<InputState>.
let search_input = cx.new(|cx| InputState::new(window, cx));

// Render: the optional ID identifies the input frame, not its state.
Input::new(&self.search_input).id("search-input")
```

`Select::new(&self.country_select)` likewise receives an
`Entity<SelectState<D>>`. `Input::new("id")` and `Select::new("id")` are not
valid constructors. See the versioned
[Input source](https://github.com/longbridge/gpui-kit/blob/1676826d9d49b90f4d7b6808b69924f58796f6f1/crates/component/src/input/input.rs)
and [Select source](https://github.com/longbridge/gpui-kit/blob/1676826d9d49b90f4d7b6808b69924f58796f6f1/crates/component/src/select.rs).

Read text through `self.search_input.read(cx).value()`. Subscribe to
`InputEvent` using `cx.subscribe_in(&self.search_input, window, ...)` when the
handler needs a window; store the returned `Subscription` on the owning view.
Dropping a constructor-local subscription stops observation. See the
[settings recipe](recipes.md) for the complete ownership pattern.

## Commands and controlled values

A Button takes a stable ID. Variants such as `primary()` require the
`ButtonVariants` trait, while size and disabled builders use `Sizable` and
`Disableable`. Import the traits supported by the particular component.

```rust
use gpui_kit::component::button::{Button, ButtonVariants};

Button::new("save").primary().label(save_label)
    .on_click(cx.listener(|this, _, window, cx| {
        this.save(window, cx);
    }))
```

Here `save_label` and `save` belong to the application. Keyboard and menu
entry points should invoke the same owner method or action; see
[actions](gpui/action.md) for the window/context callback contract.

Checkbox, Switch and Radio receive the owner's `checked(bool)` value and
request changes through `on_change`. The callback must update that owner and
notify; rebuilding the element alone does not persist a change. RadioGroup
uses an optional selected index, which the application maps to domain identity.

## Composition and theme

- `Form::new()` and `Field::new()` are value-like layout parts. Label orientation
  and column count are independent; see [conventions](conventions.md).
- `TabBar::new("tabs")` owns a collection of `Tab::new().label(...)` parts.
  `Tab::new` takes no ID argument in 0.8.0; do not infer constructors from Button.
- Use `ActiveTheme` to read `cx.theme()` semantic tokens, and `Theme::update`
  for global edits so color/token projections stay synchronized.
- `h_flex()` centers its children across the row; full-height columns may need
  `items_stretch()`. `v_flex()` leaves the cross-axis stretch default.
- Import `WindowExt` for `window.open_dialog`, `open_sheet` and
  `push_notification`. Retain owner state and restore focus on dismissal.

Use the [current component catalog](https://gpui-kit.com/llms.txt) to locate a
component page, then check signatures in the installed 0.8.0 source before
copying newer documentation. For layout, state and lifecycle rules follow the
normative guides; for verification in this repository follow
[docs/TESTING.md](../../../../docs/TESTING.md) and `./scripts/precheck.sh`.

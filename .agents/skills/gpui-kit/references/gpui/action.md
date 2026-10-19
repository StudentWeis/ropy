# Actions and keybindings

**Contents:** [Ownership](#ownership) · [Handler contract](#handler-contract) · [Dispatch and menus](#dispatch-and-menus)

## Ownership

Use an Action for a command shared by keyboard, menu, and pointer entry points.
Define unit actions with `gpui_kit::actions!`, register `KeyBinding`s on `App`,
and put `key_context`, `track_focus`, and `on_action` on the region that owns the
command. The binding is useful only when that region is on the focus path.
See the normative [events, actions, and focus guide](../coding-guides.md#events-actions-and-focus).

## Handler contract

For the repository's `gpui-kit` 0.8.0 / `gpui-pre` 0.3.9 dependency,
`Context<T>::listener` accepts
`Fn(&mut T, &E, &mut Window, &mut Context<T>) + 'static` and returns an element
callback accepting `(&E, &mut Window, &mut App)`. A method passed to
`cx.listener(Self::save)` therefore needs the Window argument, even if unused.

This is a fragment inside an existing view, not a standalone application:

```rust
fn save(&mut self, _: &Save, _window: &mut Window, cx: &mut Context<Self>) {
    self.save_document();
    cx.notify();
}
```

The corresponding render chain is `.on_action(cx.listener(Self::save))`.
Use the supplied `this`/`&mut self` in a listener; do not capture a render-time
borrow of the view in the callback. The [locked listener implementation](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app/context.rs)
and [element action handling](https://docs.rs/crate/gpui-pre/0.3.9/source/src/elements/div.rs)
are the authority for callback signatures and dispatch phases.

## Dispatch and menus

Keep one owner method for each command and route each entry point through it.
Only stop propagation when the interaction must prevent an ancestor from
handling the same event. Test both keyboard and pointer entry points through
the [UI integration workflow](test.md).

Bind keys before calling `cx.set_menus`: native menu shortcuts are derived from
the keymap when menus are built. Rebuild menus after changing bindings or menu
labels. Check platform key names against the locked implementation instead of
assuming `cmd` means Ctrl on every platform. Application quit/close commands
and their confirmation policy belong to the application.

See [focus handling](focus-handle.md) for retained handles, tab stops, and focus
restoration, and the [versioned App source](https://docs.rs/crate/gpui-pre/0.3.9/source/src/app.rs)
for menu and binding APIs.

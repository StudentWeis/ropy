# Upstream application recipes

The application recipes belong to the **GPUI Kit upstream repository**, not
Ropy. Use the sources pinned to the revision published as `gpui-kit` 0.8.0:

- [Bootstrap](https://github.com/longbridge/gpui-kit/blob/1676826d9d49b90f4d7b6808b69924f58796f6f1/examples/ai_recipes/src/bootstrap.rs): installs assets, initializes Kit, opens a window and implements a minimal `Render` view.
- [Settings view](https://github.com/longbridge/gpui-kit/blob/1676826d9d49b90f4d7b6808b69924f58796f6f1/examples/ai_recipes/src/settings.rs): retains an input entity and its subscription, handles controlled values, and opens a dialog through `WindowExt`.
- [Recipe package](https://github.com/longbridge/gpui-kit/blob/1676826d9d49b90f4d7b6808b69924f58796f6f1/examples/ai_recipes): complete crate context and its tests.

In the settings recipe, state and subscriptions live in the view. Rendering
creates only elements. The window's `Root` owns the dialog, sheet and
notification layers. A constructor-local subscription would be dropped on
return; retaining it is part of the example's behavior.

The commands `cargo test -p gpui-kit-recipes`, `script/check-ai-recipes` and
`script/check-ai {docs,rust,shell,all}` are **upstream checkout commands**.
Those packages and scripts are not present in Ropy. The pinned recipe sources
were inspected for this reference; Ropy's precheck does not compile those
external examples or certify their current upstream CI status.

For Ropy changes use [the local testing guide](../../../../docs/TESTING.md)
and `./scripts/precheck.sh`. Existing application setup is in
[src/gui/window.rs](../../../../src/gui/window.rs); production-view test
patterns are in [src/gui/board/ui_tests.rs](../../../../src/gui/board/ui_tests.rs).
Use the current [Getting Started](https://gpui-kit.com/docs/getting-started.md)
guide for a new application, verifying signatures against the chosen lockfile.

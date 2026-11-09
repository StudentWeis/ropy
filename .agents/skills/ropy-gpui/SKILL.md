---
name: ropy-gpui
description: Ropy-specific integration for the official gpui-kit and gpui-kit-design-guides skills. Use alongside them when developing, reviewing or testing Ropy UI, or upgrading GPUI Kit. Covers the locked backend, verified API differences, localization and repository verification commands.
---

# Ropy GPUI adapter

Read the official [GPUI Kit skill](../gpui-kit/SKILL.md), and the official [Design Guides skill](../gpui-kit-design-guides/SKILL.md) for visible UI work. Before applying their examples or testing advice, read the [locked API notes](references/locked-api.md), including the corrections for Masonry child identity, Transaction rollback, entity borrow chains and animation clocks. These notes are required for ordinary development and tests, not only upgrades. The installed source and lockfile determine actual API signatures; repository instructions determine Ropy conventions.

## Official snapshots

Both official skill directories are unmodified copies of [GPUI Kit v0.9.1](https://github.com/longbridge/gpui-kit/tree/v0.9.1/skills), commit `39ab849a93b5f15507b969164d49891db8d5b470`. Keep all local additions and corrections in this adapter, including changes to links and examples. Do not edit or add files inside either official directory. When upgrading, replace each complete directory from one upstream revision and record the new revision here.

To verify against an extracted upstream checkout, run `diff -qr <upstream>/skills/gpui-kit .agents/skills/gpui-kit` and the equivalent command for `gpui-kit-design-guides`. Both must report no differences.

## Ropy contracts

- Normal and development dependencies use `gpui-kit` 0.9.1, with the default `gpui-pre` 0.3.12 engine. Keep the existing feature set unless the task requires a change. Optional backend entries in Cargo.lock do not imply an active renderer switch.
- Localize visible strings and accessible names with existing translation keys where appropriate. The official design guide's English-only accessibility-label exception does not override Ropy's localization convention.
- The Coding Guides allow deliberately record-like configuration, theme tokens, geometry and serialized schemas to expose fields with `#[non_exhaustive]` and construction APIs. Follow that detailed rule instead of treating the official entrypoint's abbreviated “No pub fields” statement as an instruction to migrate unrelated types.
- `gpui_kit::open_window` installs the window Root and returns the window handle and view entity. Ropy's current [app.rs](../../../src/gui/app.rs) instead uses `cx.open_window` and explicitly constructs `Root::new` once. Do not add a second Root or duplicate dialog, sheet or notification layers. [window.rs](../../../src/gui/window.rs) manages the window handle, visibility, dismissal and recreation lifecycle; it is not the Root construction site.
- Upstream recipes and `script/check-ai` commands belong to the upstream checkout. Ropy uses [docs/testing.md](../../../docs/testing.md) and `./scripts/precheck.sh`; its gate does not compile Markdown snippets or certify upstream recipes.
- Headless UI tests exercise real components through the test platform. Native clipboard, OS IME, accessibility delivery and platform window behavior still require native acceptance evidence.

## Task-specific references

- For custom layout or Masonry work, read [Masonry child mapping](references/locked-api.md#masonry-child-mapping) before the upstream advanced Element example; its column-local indices and bounds order are not safe to copy.
- For entity state changes, read [entity borrow chains](references/locked-api.md#entity-borrow-chains) and [Transaction error paths](references/locked-api.md#transaction-error-paths). External entity reads are not all re-entrant, and the upstream Transaction example does not roll back on Drop or `?`.
- Before writing animation tests or applying the official testing reference, read [animation timing and tests](references/locked-api.md#animation-timing-and-tests). The locked engine uses the executor clock, including non-synced Animation; the upstream wall-clock advice does not apply.
- For 0.9.1 adoption and prioritized project opportunities, read [integration notes](references/integration.md).
- Production-view UI integration tests live in [ui_tests.rs](../../../src/gui/board/ui_tests.rs). Use the official testing reference for the new `TestQueryExt`, `TestEventExt` and `TestInput` APIs, checking signatures in the installed crate.

Official site-relative links and diagrams remain unchanged in the upstream copies. Resolve them against `https://gpui-kit.com`; a missing file in Ropy does not authorize inventing a local counterpart.

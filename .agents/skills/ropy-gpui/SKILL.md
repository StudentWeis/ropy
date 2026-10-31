---
name: ropy-gpui
description: Ropy-specific integration for the official gpui-kit and gpui-kit-design-guides skills. Use alongside them when developing, reviewing or testing Ropy UI, or upgrading GPUI Kit. Covers the locked backend, verified API differences, localization and repository verification commands.
---

# Ropy GPUI adapter

Read the official [GPUI Kit skill](../gpui-kit/SKILL.md), and the official [Design Guides skill](../gpui-kit-design-guides/SKILL.md) for visible UI work. Apply the repository-specific constraints below alongside them. The installed source and lockfile determine actual API signatures; repository instructions determine Ropy conventions.

## Official snapshots

Both official skill directories are unmodified copies of [GPUI Kit v0.8.1](https://github.com/longbridge/gpui-kit/tree/v0.8.1/skills), commit `99576db58ec8103417c41a092e891b4022d24e7e`. Keep all local additions and corrections in this adapter, including changes to links and examples. Do not edit or add files inside either official directory. When upgrading, replace each complete directory from one upstream revision and record the new revision here.

To verify against an extracted upstream checkout, run `diff -qr <upstream>/skills/gpui-kit .agents/skills/gpui-kit` and the equivalent command for `gpui-kit-design-guides`. Both must report no differences.

## Ropy contracts

- Normal and development dependencies use `gpui-kit` 0.8.1, with the default `gpui-pre` 0.3.10 engine. Keep the existing feature set unless the task requires a change. Optional backend entries in Cargo.lock do not imply an active renderer switch.
- Localize visible strings and accessible names with existing translation keys where appropriate. The official design guide's English-only accessibility-label exception does not override Ropy's localization convention.
- The Coding Guides allow deliberately record-like configuration, theme tokens, geometry and serialized schemas to expose fields with `#[non_exhaustive]` and construction APIs. Follow that detailed rule instead of treating the official entrypoint's abbreviated “No pub fields” statement as an instruction to migrate unrelated types.
- `gpui_kit::open_window` installs the window Root and returns the window handle and view entity. Do not mount duplicate dialog, sheet or notification layers. Existing setup is in [window.rs](../../../src/gui/window.rs).
- Upstream recipes and `script/check-ai` commands belong to the upstream checkout. Ropy uses [docs/TESTING.md](../../../docs/TESTING.md) and `./scripts/precheck.sh`; its gate does not compile Markdown snippets or certify upstream recipes.
- Headless UI tests exercise real components through the test platform. Native clipboard, OS IME, accessibility delivery and platform window behavior still require native acceptance evidence.

## Task-specific references

- Before copying low-level examples, read [locked API notes](references/locked-api.md) for callback arity, entity return types, focus and Element lifecycle differences.
- For 0.8.1 adoption and prioritized project opportunities, read [integration notes](references/integration.md).
- Production-view UI integration tests live in [ui_tests.rs](../../../src/gui/board/ui_tests.rs). Use the official testing reference for the new `TestQueryExt`, `TestEventExt` and `TestInput` APIs, checking signatures in the installed crate.

Official site-relative links and diagrams remain unchanged in the upstream copies. Resolve them against `https://gpui-kit.com`; a missing file in Ropy does not authorize inventing a local counterpart.

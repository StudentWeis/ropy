# Ropy integration with GPUI Kit 0.8.0

## Source and refresh policy

Checked against [GPUI Kit v0.8.0](https://github.com/longbridge/gpui-kit/releases/tag/v0.8.0), commit `1676826d9d49b90f4d7b6808b69924f58796f6f1`. The upstream default branch matched this release when checked on 2026-10-19. Normal and development dependencies use 0.8.0; the default engine remains `gpui-pre` 0.3.9. Optional GPUI Fast entries in Cargo.lock do not mean Ropy switched rendering engines.

Use this skill with `gpui-kit-design-guides` for visible UI work. First read the relevant guide sections, then the component family convention, then verify the exact signature in the installed source. Keep Ropy's local API corrections, pinned recipe links and test commands when refreshing upstream material. Upstream `script/check-ai` commands do not run in this repository.

This reference adapts [GPUI Kit documentation](https://gpui-kit.com) under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/), adding Ropy-specific scope, verification and recommendations. Code examples retain their upstream Apache-2.0 terms.

## Adopted integration

- `Slider::accessibility_label` names the opacity slider and each RGB channel using existing localized labels. The settings UI integration test queries the opacity label through `TestQueryExt` and checks that it identifies a slider.
- `TestQueryExt` provides accessible-label and role queries. `TestEventExt` supports individual pointer/key transitions. `TestInput` exercises an explicitly supplied input handler's composition protocol. See [Testing](gpui/test.md) for their contracts and limits.
- Input caret/reflow, Select keyboard reconciliation, Popover keyboard activation and Tooltip dismissal fixes arrive through the component upgrade. They do not require duplicate application workarounds.
- Keep all accessible names localized in Ropy, even though the upstream design guide now permits English-only accessibility strings.

## Optimization choices

| Priority | Ropy surface | Recommendation and evidence needed |
| --- | --- | --- |
| Next | Search input and Chinese text entry | Use `TestInput` with the production search input to cover preedit, commit and cancellation. Still verify native IME candidate windows and focus separately; the helper does not run an OS IME. |
| Next | Hold-Space preview and modified gestures | Use `TestEventExt` or `click_with_modifiers` when adding regression cases, reducing manually constructed event boilerplate while keeping held-key and focus-loss assertions. Do not rewrite passing tests solely for style. |
| Evaluate | History list and masonry scrolling | The custom history layouts own virtualized scroll state. Verify trackpad edge behavior, selection visibility and scrollbar geometry before adopting the shared boundary-bounce behavior; the upgrade alone does not prove custom surfaces inherit it. |
| Conditional | Long text preview | Consider Textarea `prewrap` only if measurement identifies repeated wrapping cost. Current previews render plain text; TextView selection fixes and `SelectionFormat::SourceAcrossStyles` are not automatic benefits for those previews. |
| Deferred | Syntax-highlighted previews | `syntect` is useful if code preview becomes a product requirement. Measure startup, memory and binary size before enabling it. Ropy currently leaves highlighting, Shell and WebView features disabled. |

Keep the existing core/UI boundary and virtualized history layout. This release does not justify a Shell rewrite, WebView dependency, renderer switch or replacement of the clipboard-specific selection model.

## Verification

Run `./scripts/precheck.sh` for formatting, resource checks, Clippy, all workspace targets/features and documentation. For a focused check run `cargo test --locked -p ropy test_board_controls_expose_localized_accessible_names`.

Headless UI integration checks exercise real components through the test platform; they do not certify macOS accessibility delivery, native clipboard/IME services, or Windows/Linux desktop behavior. Follow [Ropy testing](../../../../docs/TESTING.md) for platform acceptance.

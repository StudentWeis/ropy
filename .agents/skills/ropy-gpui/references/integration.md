# Ropy integration with GPUI Kit 0.10.0

## Source and refresh policy

Checked against [GPUI Kit v0.10.0](https://github.com/longbridge/gpui-kit/releases/tag/v0.10.0), commit `4e015e9e26e137e020ebc0c4fecf9ce5e0889668`. Normal and development dependencies use 0.10.0; the default engine is `gpui-pre` 0.3.12. Optional GPUI Fast entries in Cargo.lock (now 0.1.16) do not mean Ropy switched rendering engines.

Use the official `gpui-kit` and `gpui-kit-design-guides` skills alongside the `ropy-gpui` adapter for visible UI work. First read the relevant guide sections, then the component family convention, then verify the exact signature in the installed source. Keep Ropy-specific guidance in `ropy-gpui`; replace the two official skill directories only with an unmodified upstream snapshot. Upstream `script/check-ai` commands do not run in this repository.

This reference adapts [GPUI Kit documentation](https://gpui-kit.com) under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/), adding Ropy-specific scope, verification and recommendations. Code examples retain their upstream Apache-2.0 terms.

## 0.10.0 migration

This is a minor upgrade with no breaking changes affecting Ropy's current API usage (`TextareaMode::Extras`, `EditorState::text_gutter_margin()`, and GPUI Fast's blanket `IntoElement` do not impact Ropy). Keep the production `component`/`assets` and development `component`/`test-support` features unchanged.

The official `gpui-kit` skill snapshot was updated to v0.10.0 (only `references/usage.md` changed); `gpui-kit-design-guides` is byte-for-byte identical to the v0.9.1 snapshot.

Ropy's `apply_palette` in `src/gui/app.rs` now sets `theme.colors.list_highlight_foreground = theme.primary` so the new 0.10.0 highlight token matches Ropy's theme palette instead of falling back to the default blue.

Through the component upgrade, Ropy automatically gains:
- Search input grapheme-cluster caret movement/deletion, caret reveal after long single-line paste, `unmark_text` change reporting for retained preedit, and IME candidate row bounds fixes.
- `Slider` keyboard shortcuts and numeric accessibility `SetValue` handling for the opacity and RGB sliders.
- `Tooltip` width animation, `max_w` text wrapping, and nested `.tooltip()` deduplication.
- Built-in reduced-motion support across `Dialog`, `Scrollbar`, and `Tooltip`, plus `Button` disabled hover/active suppression and right/middle click focus prevention.

In `src/gui/board/ui_tests.rs`, `test_board_search_input_ime_composition_commit_cancel_and_unmark` now exercises `TestInput` against the production `search_input` for Chinese preedit, commit, cancellation, and `unmark_text` filtering.

## 0.9.1 migration

This is a backward-compatible patch upgrade. Keep the production `component`/`assets` and development `component`/`test-support` features unchanged. The lockfile also updates optional GPUI Fast packages to 0.1.14; their frame-reuse improvement is not a benefit of Ropy's active `gpui-pre` backend.

Both official skill directories in v0.9.1 are byte-for-byte identical to the recorded v0.9.0 snapshot. Verified with `diff -qr` against the tagged upstream archive; only the adapter's recorded revision changes.

Ropy's search input and theme-name inputs use `InputState`, so upstream caret movement, prepaint layout publication, focused-only layout notifications and early-value shaping fixes apply without application workarounds. Verify native Chinese IME positioning separately: headless tests cannot certify the OS candidate window.

The opacity and RGB controls use `Slider`. They inherit fixes for step snapping from `min`, disabled-thumb accessibility and assistive-technology stepping. Checked-menu positioning and rem-based menu sizing also arrive through the shared components. Custom history scrolling still needs its own acceptance evidence.

## 0.9.0 migration

Public upstream theme structs are now `#[non_exhaustive]`. Construct them with their existing constructors or `Default::default()`, then assign fields; struct literals with `..Default::default()` are no longer supported. Ropy's existing theme construction already follows this pattern.

`Window::text_system()` now returns `&Rc<WindowTextSystem>` rather than `&Arc<WindowTextSystem>`, and the text system is no longer Send/Sync. Animation timing now follows the executor clock; headless tests should advance that clock rather than sleep on the wall clock. No application-side API migration was needed for these changes.

The official skill directories in v0.9.0 are byte-for-byte identical to the previously recorded v0.8.1 snapshot. Their recorded source revision advances with the dependency.

## Previous migration

Dynamic shortcut loading uses `KeyBinding::load` with the application context. Parsing and formatting use `cx.platform_info().os`; this keeps shortcut validation, recording and display consistent with the runtime host. Static default bindings use string literals. The input-blocking promotion overlay uses the renamed `on_scroll` handler.

Existing UI integration tests cover shortcut editing, persistence and focus; host-parameterized binding tests cover `secondary` normalization and conflicts on macOS, Windows and Linux.

## Adopted integration

- `Slider::accessibility_label` names the opacity slider and each RGB channel using existing localized labels. The settings UI integration test queries the opacity label through `TestQueryExt` and checks that it identifies a slider.
- `TestQueryExt` provides accessible-label and role queries. `TestEventExt` supports individual pointer/key transitions. `TestInput` exercises an explicitly supplied input handler's composition protocol. See [Testing](../../gpui-kit/references/gpui/test.md) for their contracts and limits.
- Input caret/reflow, Select keyboard reconciliation, Popover keyboard activation and Tooltip dismissal fixes arrive through the component upgrade. They do not require duplicate application workarounds.
- Keep all accessible names localized in Ropy, even though the upstream design guide now permits English-only accessibility strings.

## Optimization choices

| Priority | Ropy surface | Recommendation and evidence needed |
| --- | --- | --- |
| Blocked upstream | Slider keyboard focus | The themed `gpui_component::slider::Slider` still exposes `tab_stop` and `tab_index` in 0.10.0, not `track_focus` (which remains on `gpui-base::slider::Slider`). Do not call the missing builder or replace the themed control just to obtain it. |
| Blocked upstream | Theme catalog selection | `gpui_component::button::Button` still has no `selected_accent` builder in 0.10.0. Keep the catalog's outline buttons with `.selected(...)`; recheck the published API before adopting accent-filled selection in a later release. |
| Covered | Search input and Chinese text entry | `TestInput` now covers preedit, commit, cancellation, and `unmark_text` on the production search input. Still verify native IME candidate windows and focus separately; the helper does not run an OS IME. |
| Next | Hold-Space preview and modified gestures | Use `TestEventExt` or `click_with_modifiers` when adding regression cases, reducing manually constructed event boilerplate while keeping held-key and focus-loss assertions. Do not rewrite passing tests solely for style. |
| Evaluate | History list and masonry scrolling | The custom history layouts own virtualized scroll state. Verify trackpad edge behavior, selection visibility and scrollbar geometry before adopting the shared boundary-bounce behavior; the upgrade alone does not prove custom surfaces inherit it. |
| Conditional | Long text preview | Consider Textarea `prewrap` only if measurement identifies repeated wrapping cost. Current previews render plain text; TextView selection fixes and `SelectionFormat::SourceAcrossStyles` are not automatic benefits for those previews. |
| Deferred | Syntax-highlighted previews | `syntect` is useful if code preview becomes a product requirement. Measure startup, memory and binary size before enabling it. Ropy currently leaves highlighting, Shell and WebView features disabled. |

Do not replace the virtualized history list or masonry layout with the new draggable `GridLayout`: dashboard drag/resize behavior is not clipboard-history virtualization. Markdown task toggles, right-pinned table columns, vertical toolbars and retained `TransitionState` have no immediate production use here. Plain-text previews do not automatically gain TextView's Markdown table, selection or link-tooltip fixes.

Keep the existing core/UI boundary and virtualized history layout. This release does not justify a Shell rewrite, WebView dependency, renderer switch or replacement of the clipboard-specific selection model.

## Verification

Run `./scripts/precheck.sh` for formatting, resource checks, Clippy, all workspace targets/features and documentation. For a focused check run `cargo test --locked -p ropy test_board_controls_expose_localized_accessible_names`.

Headless UI integration checks exercise real components through the test platform; they do not certify macOS accessibility delivery, native clipboard/IME services, or Windows/Linux desktop behavior. Follow [Ropy testing](../../../../docs/testing.md) for platform acceptance.

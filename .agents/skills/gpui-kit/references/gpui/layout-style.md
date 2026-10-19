# Layout and styling

Follow the normative [theme and styling](../coding-guides.md#theme-and-styling)
and [layout and measurement](../coding-guides.md#layout-measurement-and-scrolling)
rules, plus the [Design Guides](../../../gpui-kit-design-guides/references/design-guides.md).
This page describes the locked Kit 0.8.0 / GPUI 0.3.9 mechanisms; it does not
supply an alternate visual system.

## Composition and units

`Styled` provides fluent layout and text helpers. Use `ActiveTheme` for semantic
colors and theme radii. Application spacing uses the rem-based scale helpers;
`p_2()` and `gap_3()` scale with the window's rem size, rather than always
meaning a fixed number of pixels. Reserve explicit pixel values for documented
physical boundaries or measured geometry.

A renderer fragment, with its component imports:

```rust
use gpui_kit::component::{ActiveTheme, v_flex};
use gpui_kit::prelude::*;

v_flex()
    .size_full()
    .gap_3()
    .p_4()
    .bg(cx.theme().background)
    .text_color(cx.theme().foreground)
    .border_1()
    .border_color(cx.theme().border)
    .rounded(cx.theme().radius)
```

Verify tokens in the installed theme type; do not invent `theme.hover` or
assume a website's newest theme API exists in the lockfile. Apply runtime
interaction refinements through supported `hover`, `active` and focus methods,
and use the component's own selected/disabled APIs for semantic states.

`h_flex()` is a row with `items_center()`; `v_flex()` is a column with the
normal stretch default. For rows containing full-height panels, set
`items_stretch()` or the child's `h_full()` deliberately. The helpers do not
make these two axes symmetrical.

## Overflow and geometry

Give each scrollable region one owner. Flexible children containing scrolling
content often need `min_w_0()` or `min_h_0()` so content can shrink. A plain
`Div` with scrolling needs stable identity; scrollbar helpers also require the
component's `ScrollableElement` extension trait. Inspect the specific helper
before assuming every overflow method exists on every element type.

Keep content inset inside the scroll owner so its scrollbar sits at the
viewport edge. Clipping can also clip an outward focus ring; leave physical
room or use the documented theme focus policy.

Prefer layout constraints over manual bounds calculations. Resolved geometry
belongs in prepaint, after request-layout has completed. Measurements become
stale when width, content, font, theme or rem size changes. See
[custom element phases](element.md) before implementing measurement.

Absolute-positioned children participate in the parent's composition; later
siblings normally paint above earlier siblings. There is no general GPUI
`Styled::z_index` method. Let Kit's `Root` own overlay layers rather than
creating ad-hoc paint-order workarounds.

## Versioned source

- [GPUI Styled helpers](https://docs.rs/crate/gpui-pre/0.3.9/source/src/styled.rs)
- [Kit styling extensions](https://github.com/longbridge/gpui-kit/blob/1676826d9d49b90f4d7b6808b69924f58796f6f1/crates/component/src/styled.rs)
- [Kit scroll extensions](https://github.com/longbridge/gpui-kit/blob/1676826d9d49b90f4d7b6808b69924f58796f6f1/crates/component/src/scroll/scrollable.rs)
- [Kit theme](https://github.com/longbridge/gpui-kit/blob/1676826d9d49b90f4d7b6808b69924f58796f6f1/crates/component/src/theme)

The fragment above was checked against these APIs; it is not a standalone
application or evidence of rendered layout quality. Test actual controls,
resizing, theme changes, focus and scrolling in the production view.

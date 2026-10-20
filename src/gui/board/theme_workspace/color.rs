//! Opaque RGB editing composed from standard Input, Popover and Slider controls.
use crate::gui::theme::document::ColorRole;
use gpui_kit::{
    App, AppContext, Context, Entity, EventEmitter, IntoElement, ParentElement, Render, Styled,
    Subscription, Window,
    component::{
        ActiveTheme, Sizable,
        button::{Button, ButtonVariants},
        h_flex,
        input::{Input, InputEvent, InputState},
        popover::Popover,
        slider::{Slider, SliderState},
        v_flex,
    },
    div, rgb,
};
use rust_i18n::t;

#[expect(clippy::redundant_pub_crate)]
pub(crate) struct ColorEditor {
    pub(crate) input: Entity<InputState>,
    sliders: [Entity<SliderState>; 3],
    role: ColorRole,
    value: u32,
    _subscriptions: Vec<Subscription>,
}

pub(super) struct ColorEdited;

impl EventEmitter<ColorEdited> for ColorEditor {}

impl ColorEditor {
    pub(crate) fn new(
        role: ColorRole,
        value: u32,
        text: Option<String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(text.unwrap_or_else(|| format!("#{value:06X}")))
        });
        let sliders = [16, 8, 0].map(|shift| {
            cx.new(|_| {
                SliderState::new()
                    .min(0.)
                    .max(255.)
                    .step(1.)
                    .default_value(f32::from(channel(value, shift)))
            })
        });
        let mut subscriptions = vec![cx.subscribe_in(
            &input,
            window,
            |editor, input, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::Focus) {
                    cx.emit(ColorEdited);
                    cx.notify();
                }
                if matches!(event, InputEvent::Change) {
                    if let Some(value) = parse_hex(&input.read(cx).value()) {
                        editor.value = value;
                        for (slider, shift) in editor.sliders.iter().zip([16, 8, 0]) {
                            slider.update(cx, |slider, cx| {
                                slider.set_value(f32::from(channel(value, shift)), window, cx);
                            });
                        }
                    }
                    cx.emit(ColorEdited);
                    cx.notify();
                }
            },
        )];
        for (slider, shift) in sliders.iter().zip([16, 8, 0]) {
            // Observe the value itself: GPUI 0.8 accessibility actions notify
            // the slider without emitting SliderEvent::Change.
            subscriptions.push(
                cx.observe_in(slider, window, move |editor, slider, window, cx| {
                    let value = slider_byte(slider.read(cx).value().end());
                    if value == channel(editor.value, shift) {
                        return;
                    }
                    editor.value = (editor.value & !(0xff << shift)) | (u32::from(value) << shift);
                    editor.input.update(cx, |input, cx| {
                        input.set_value(format!("#{:06X}", editor.value), window, cx);
                    });
                    cx.emit(ColorEdited);
                    cx.notify();
                }),
            );
        }
        Self {
            input,
            sliders,
            role,
            value,
            _subscriptions: subscriptions,
        }
    }

    pub(crate) const fn preview(&self) -> u32 {
        self.value
    }

    pub(crate) fn value(&self, cx: &App) -> Option<u32> {
        parse_hex(&self.input.read(cx).value())
    }
}

impl Render for ColorEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let label = t!(self.role.label_key());
        let sliders = self.sliders.clone();
        let role = self.role;
        let owner = cx.entity().downgrade();
        let popup = Popover::new(("theme-color-popup", cx.entity_id()))
            .on_open_change(move |open, _, cx| {
                if *open {
                    let _ = owner.update(cx, |_, cx| {
                        cx.emit(ColorEdited);
                        cx.notify();
                    });
                }
            })
            .trigger(
                Button::new("theme-color-swatch")
                    .small()
                    .ghost()
                    .accessibility_label(t!("themes_adjust_color", color = label))
                    .child(
                        div()
                            .size_4()
                            .rounded_sm()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(rgb(self.value)),
                    ),
            )
            .content(move |_, _, cx| {
                v_flex()
                    .w(gpui_kit::rems(14.))
                    .gap_3()
                    .p_3()
                    .child(div().text_sm().child(t!(role.label_key())))
                    .children(
                        sliders
                            .iter()
                            .zip([t!("themes_red"), t!("themes_green"), t!("themes_blue")])
                            .map(|(slider, label)| {
                                v_flex()
                                    .gap_1()
                                    .child(
                                        h_flex()
                                            .justify_between()
                                            .text_sm()
                                            .child(label.clone())
                                            .child(format!("{:.0}", slider.read(cx).value().end())),
                                    )
                                    .child(Slider::new(slider).accessibility_label(label))
                            }),
                    )
            });
        Input::new(&self.input)
            .small()
            .w_full()
            .aria_label(label)
            .suffix(popup)
            .border_color(if self.value(cx).is_some() {
                cx.theme().input
            } else {
                cx.theme().danger
            })
    }
}

fn parse_hex(text: &str) -> Option<u32> {
    let text = text.trim();
    let text = text.strip_prefix('#').unwrap_or(text);
    (text.len() == 6 && text.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .then(|| u32::from_str_radix(text, 16).ok())
        .flatten()
}

const fn channel(value: u32, shift: u32) -> u8 {
    value.to_be_bytes()[match shift {
        16 => 1,
        8 => 2,
        _ => 3,
    }]
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Slider values are rounded and bounded to a byte"
)]
const fn slider_byte(value: f32) -> u8 {
    value.round().clamp(0., 255.) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[gpui_kit::test]
    #[expect(
        clippy::unwrap_used,
        reason = "test setup and window updates must succeed"
    )]
    fn test_rgb_slider_state_change_without_change_event_updates_color(
        cx: &mut gpui_kit::TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        let (handle, editor) = cx.update(|cx| {
            gpui_kit::open_window(gpui_kit::WindowOptions::default(), cx, |window, cx| {
                cx.new(|cx| ColorEditor::new(ColorRole::Background, 0x0012_3456, None, window, cx))
            })
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            let red = editor.read(cx).sliders[0].clone();
            // Use the same public setter as the locked Slider accessibility action.
            red.update(cx, |slider, cx| slider.set_value(19., window, cx));
        })
        .unwrap();
        cx.run_until_parked();
        cx.read(|cx| {
            assert_eq!(editor.read(cx).preview(), 0x0013_3456);
            assert_eq!(editor.read(cx).value(cx), Some(0x0013_3456));
        });
    }

    #[test]
    fn test_rgb_input_accepts_exact_color_and_rejects_alpha() {
        assert_eq!(parse_hex("#123456"), Some(0x0012_3456));
        assert_eq!(parse_hex("ABCDEF"), Some(0x00ab_cdef));
        for invalid in ["#12345680", "rgba(1,2,3,.5)", "#123", "#zzzzzz", ""] {
            assert_eq!(parse_hex(invalid), None);
        }
    }
}

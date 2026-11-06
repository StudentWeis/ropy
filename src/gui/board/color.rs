use gpui_kit::Rgba;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ClipboardColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

impl ClipboardColor {
    #[cfg(test)]
    pub(super) const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub(super) fn to_gpui_rgba(self) -> Rgba {
        gpui_kit::rgba(self.to_rgba_u32())
    }

    pub(super) const fn to_rgba_u32(self) -> u32 {
        u32::from_be_bytes([self.red, self.green, self.blue, self.alpha])
    }
}

pub(super) fn parse_clipboard_color(content: &str) -> Option<ClipboardColor> {
    let trimmed = content.trim();

    if trimmed.starts_with('#') {
        return parse_hex_color(trimmed);
    }

    parse_function_color(trimmed)
}

fn parse_hex_color(content: &str) -> Option<ClipboardColor> {
    let hex = content.strip_prefix('#')?;

    match hex.len() {
        3 if hex.chars().all(|ch| ch.is_ascii_hexdigit()) => Some(ClipboardColor {
            red: expand_hex_digit(&hex[0..1])?,
            green: expand_hex_digit(&hex[1..2])?,
            blue: expand_hex_digit(&hex[2..3])?,
            alpha: u8::MAX,
        }),
        4 if hex.chars().all(|ch| ch.is_ascii_hexdigit()) => Some(ClipboardColor {
            red: expand_hex_digit(&hex[0..1])?,
            green: expand_hex_digit(&hex[1..2])?,
            blue: expand_hex_digit(&hex[2..3])?,
            alpha: expand_hex_digit(&hex[3..4])?,
        }),
        6 if hex.chars().all(|ch| ch.is_ascii_hexdigit()) => Some(ClipboardColor {
            red: parse_hex_byte(&hex[0..2])?,
            green: parse_hex_byte(&hex[2..4])?,
            blue: parse_hex_byte(&hex[4..6])?,
            alpha: u8::MAX,
        }),
        8 if hex.chars().all(|ch| ch.is_ascii_hexdigit()) => Some(ClipboardColor {
            red: parse_hex_byte(&hex[0..2])?,
            green: parse_hex_byte(&hex[2..4])?,
            blue: parse_hex_byte(&hex[4..6])?,
            alpha: parse_hex_byte(&hex[6..8])?,
        }),
        _ => None,
    }
}

fn parse_function_color(content: &str) -> Option<ClipboardColor> {
    let open_paren = content.find('(')?;
    let name = &content[..open_paren];
    let parse: fn(&[&str]) -> Option<ClipboardColor> = if name.eq_ignore_ascii_case("rgb") {
        parse_rgb_function
    } else if name.eq_ignore_ascii_case("rgba") {
        parse_rgba_function
    } else if name.eq_ignore_ascii_case("hsl") {
        parse_hsl_function
    } else if name.eq_ignore_ascii_case("hsla") {
        parse_hsla_function
    } else {
        return None;
    };
    let arguments = content[open_paren + 1..].strip_suffix(')')?;
    let (args, count) = split_function_arguments(arguments)?;
    parse(&args[..count])
}

fn split_function_arguments(arguments: &str) -> Option<([&str; 4], usize)> {
    let mut values = [""; 4];
    let mut count = 0;
    // The fifth segment only detects excess arguments. Never allocate based on
    // clipboard text, or scan the rest of a comma-heavy non-color payload.
    for (ix, value) in arguments.splitn(5, ',').enumerate() {
        let slot = values.get_mut(ix)?;
        *slot = value.trim();
        if slot.is_empty() {
            return None;
        }
        count = ix + 1;
    }
    Some((values, count))
}

fn parse_rgb_function(args: &[&str]) -> Option<ClipboardColor> {
    if args.len() != 3 {
        return None;
    }

    Some(ClipboardColor {
        red: parse_byte(args[0])?,
        green: parse_byte(args[1])?,
        blue: parse_byte(args[2])?,
        alpha: u8::MAX,
    })
}

fn parse_rgba_function(args: &[&str]) -> Option<ClipboardColor> {
    if args.len() != 4 {
        return None;
    }

    Some(ClipboardColor {
        red: parse_byte(args[0])?,
        green: parse_byte(args[1])?,
        blue: parse_byte(args[2])?,
        alpha: parse_alpha(args[3])?,
    })
}

fn parse_hsl_function(args: &[&str]) -> Option<ClipboardColor> {
    if args.len() != 3 {
        return None;
    }

    let (red, green, blue) = hsl_to_rgb(
        parse_hue(args[0])?,
        parse_percentage(args[1])?,
        parse_percentage(args[2])?,
    );

    Some(ClipboardColor {
        red,
        green,
        blue,
        alpha: u8::MAX,
    })
}

fn parse_hsla_function(args: &[&str]) -> Option<ClipboardColor> {
    if args.len() != 4 {
        return None;
    }

    let (red, green, blue) = hsl_to_rgb(
        parse_hue(args[0])?,
        parse_percentage(args[1])?,
        parse_percentage(args[2])?,
    );

    Some(ClipboardColor {
        red,
        green,
        blue,
        alpha: parse_alpha(args[3])?,
    })
}

fn expand_hex_digit(value: &str) -> Option<u8> {
    u8::from_str_radix(value, 16).ok().map(|digit| digit * 17)
}

fn parse_hex_byte(value: &str) -> Option<u8> {
    u8::from_str_radix(value, 16).ok()
}

fn parse_byte(value: &str) -> Option<u8> {
    value.parse::<u8>().ok()
}

fn parse_hue(value: &str) -> Option<f32> {
    value
        .parse::<f32>()
        .ok()
        .filter(|hue| hue.is_finite())
        .map(|hue| hue.rem_euclid(360.0))
}

fn parse_percentage(value: &str) -> Option<f32> {
    let percent = value.strip_suffix('%')?.parse::<f32>().ok()?;
    if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
        return None;
    }

    Some(percent / 100.0)
}

fn parse_alpha(value: &str) -> Option<u8> {
    let alpha = value.parse::<f32>().ok()?;
    if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
        return None;
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "guard above pins alpha to [0.0, 1.0] so the scaled value fits in u8"
    )]
    let byte = (alpha * 255.0).round() as u8;
    Some(byte)
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> (u8, u8, u8) {
    let chroma = (1.0 - 2.0_f32.mul_add(lightness, -1.0).abs()) * saturation;
    let hue_segment = hue / 60.0;
    let secondary = chroma * (1.0 - ((hue_segment.rem_euclid(2.0)) - 1.0).abs());

    let (red, green, blue) = if hue_segment < 1.0 {
        (chroma, secondary, 0.0)
    } else if hue_segment < 2.0 {
        (secondary, chroma, 0.0)
    } else if hue_segment < 3.0 {
        (0.0, chroma, secondary)
    } else if hue_segment < 4.0 {
        (0.0, secondary, chroma)
    } else if hue_segment < 5.0 {
        (secondary, 0.0, chroma)
    } else {
        (chroma, 0.0, secondary)
    };

    let match_value = lightness - chroma / 2.0;

    (
        float_channel_to_u8(red + match_value),
        float_channel_to_u8(green + match_value),
        float_channel_to_u8(blue + match_value),
    )
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "clamp(0.0, 1.0) keeps the scaled value in [0.0, 255.0]"
)]
fn float_channel_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{ClipboardColor, parse_clipboard_color};

    #[test]
    fn test_color_arguments_excessive_count_returns_none() {
        let arguments = format!("{}0", "0,".repeat(100_000));
        assert!(super::split_function_arguments(&arguments).is_none());
        assert_eq!(parse_clipboard_color(&format!("rgb({arguments})")), None);
        assert_eq!(
            parse_clipboard_color(&format!("unknown({arguments})")),
            None
        );
    }

    #[rstest]
    #[case("#abc", ClipboardColor::new(0xAA, 0xBB, 0xCC, 0xFF))]
    #[case("#abcd", ClipboardColor::new(0xAA, 0xBB, 0xCC, 0xDD))]
    #[case("#A1b2C3", ClipboardColor::new(0xA1, 0xB2, 0xC3, 0xFF))]
    #[case("#A1b2C3d4", ClipboardColor::new(0xA1, 0xB2, 0xC3, 0xD4))]
    #[case(" rgb(255, 0, 0) ", ClipboardColor::new(0xFF, 0x00, 0x00, 0xFF))]
    #[case("RgBa(255, 0, 0, 0.5)", ClipboardColor::new(0xFF, 0x00, 0x00, 0x80))]
    #[case("hsl(120, 100%, 50%)", ClipboardColor::new(0x00, 0xFF, 0x00, 0xFF))]
    #[case(
        "HSLA(240, 100%, 50%, 0.25)",
        ClipboardColor::new(0x00, 0x00, 0xFF, 0x40)
    )]
    fn test_parse_clipboard_color_when_supported_syntax_returns_color(
        #[case] input: &str,
        #[case] expected: ClipboardColor,
    ) {
        assert_eq!(parse_clipboard_color(input), Some(expected));
    }

    #[rstest]
    #[case("abc")]
    #[case("ff00aa")]
    #[case("rgb(100%, 0%, 0%)")]
    #[case("rgb(255 0 0 / 50%)")]
    #[case("rebeccapurple")]
    #[case("Color: #ff0000")]
    #[case("rgba(255, 0, 0, 1.5)")]
    #[case("hsl(120, 100, 50)")]
    fn test_parse_clipboard_color_when_syntax_is_unsupported_returns_none(#[case] input: &str) {
        assert_eq!(parse_clipboard_color(input), None);
    }
}

use std::str::FromStr;

use global_hotkey::hotkey::HotKey;
use gpui::{Keystroke, Modifiers};

pub fn keystroke_to_hotkey(keystroke: &Keystroke) -> Option<String> {
    if !has_supported_modifier(keystroke.modifiers) {
        return None;
    }

    let key = normalize_key(&keystroke.key)?;
    if is_modifier_key(&key) {
        return None;
    }

    let mut parts = Vec::with_capacity(6);

    #[cfg(target_os = "macos")]
    if keystroke.modifiers.platform {
        parts.push("cmd");
    }

    #[cfg(not(target_os = "macos"))]
    if keystroke.modifiers.platform {
        parts.push("super");
    }

    if keystroke.modifiers.control {
        parts.push(control_token());
    }
    if keystroke.modifiers.alt {
        parts.push("alt");
    }
    if keystroke.modifiers.shift {
        parts.push("shift");
    }
    if keystroke.modifiers.function {
        parts.push("fn");
    }

    parts.push(key.as_str());
    Some(parts.join("+"))
}

pub fn is_cancel_key(key: &str) -> bool {
    matches!(key.trim().to_ascii_lowercase().as_str(), "escape" | "esc")
}

pub fn is_clear_key(keystroke: &Keystroke) -> bool {
    !has_supported_modifier(keystroke.modifiers)
        && matches!(
            keystroke.key.trim().to_ascii_lowercase().as_str(),
            "backspace" | "delete"
        )
}

const fn has_supported_modifier(modifiers: Modifiers) -> bool {
    modifiers.platform
        || modifiers.control
        || modifiers.alt
        || modifiers.shift
        || modifiers.function
}

const fn control_token() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "control"
    }

    #[cfg(not(target_os = "macos"))]
    {
        "ctrl"
    }
}

fn normalize_key(key: &str) -> Option<String> {
    let normalized = key.trim().to_ascii_lowercase();
    let mapped = match normalized.as_str() {
        "" => return None,
        " " => "space",
        "arrowup" => "up",
        "arrowdown" => "down",
        "arrowleft" => "left",
        "arrowright" => "right",
        "return" => "enter",
        "escape" => "esc",
        other => other,
    };

    if mapped.len() == 1 || is_named_key_supported(mapped) {
        let candidate = mapped.to_string();
        if HotKey::from_str(&format!("{}+{}", control_token(), candidate)).is_ok() {
            return Some(candidate);
        }
    }

    None
}

fn is_named_key_supported(key: &str) -> bool {
    matches!(
        key,
        "space"
            | "enter"
            | "tab"
            | "backspace"
            | "delete"
            | "insert"
            | "home"
            | "end"
            | "pageup"
            | "pagedown"
            | "up"
            | "down"
            | "left"
            | "right"
            | "plus"
            | "minus"
            | "comma"
            | "period"
            | "slash"
            | "backslash"
            | "semicolon"
            | "quote"
            | "backquote"
            | "equal"
            | "capslock"
    ) || is_function_key(key)
}

fn is_function_key(key: &str) -> bool {
    let Some(rest) = key.strip_prefix('f') else {
        return false;
    };
    matches!(rest.parse::<u8>(), Ok(1..=12))
}

fn is_modifier_key(key: &str) -> bool {
    matches!(
        key,
        "shift" | "control" | "ctrl" | "alt" | "cmd" | "command" | "super" | "fn"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keystroke(key: &str, modifiers: Modifiers) -> Keystroke {
        Keystroke {
            modifiers,
            key: key.to_string(),
            key_char: None,
        }
    }

    #[test]
    fn modifier_only_keystroke_is_rejected() {
        let mut modifiers = Modifiers::none();
        modifiers.control = true;

        assert!(keystroke_to_hotkey(&keystroke("control", modifiers)).is_none());
    }

    #[test]
    fn clear_key_requires_no_modifiers() {
        let mut modifiers = Modifiers::none();
        modifiers.control = true;

        assert!(is_clear_key(&keystroke("delete", Modifiers::none())));
        assert!(!is_clear_key(&keystroke("delete", modifiers)));
    }

    #[test]
    fn escape_variants_cancel_recording() {
        assert!(is_cancel_key("escape"));
        assert!(is_cancel_key("esc"));
        assert!(!is_cancel_key("enter"));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn control_shift_letter_round_trips_to_global_hotkey() {
        let mut modifiers = Modifiers::none();
        modifiers.control = true;
        modifiers.shift = true;

        let hotkey = keystroke_to_hotkey(&keystroke("d", modifiers)).unwrap();
        assert!(HotKey::from_str(&hotkey).is_ok());
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[allow(clippy::unwrap_used)]
    fn command_shift_letter_round_trips_to_global_hotkey() {
        let mut modifiers = Modifiers::none();
        modifiers.platform = true;
        modifiers.shift = true;

        let hotkey = keystroke_to_hotkey(&keystroke("v", modifiers)).unwrap();
        assert_eq!(hotkey, "cmd+shift+v");
        assert!(HotKey::from_str(&hotkey).is_ok());
    }
}

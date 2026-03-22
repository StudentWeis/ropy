use std::str::FromStr;

use global_hotkey::hotkey::HotKey;
use gpui::{Context, Focusable, Keystroke, Modifiers, Window};

use super::RopyBoard;

// --- Hotkey recording utilities (merged from gui/hotkey_record.rs) ---

fn keystroke_to_hotkey(keystroke: &Keystroke) -> Option<String> {
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

fn is_cancel_key(key: &str) -> bool {
    matches!(key.trim().to_ascii_lowercase().as_str(), "escape" | "esc")
}

fn is_clear_key(keystroke: &Keystroke) -> bool {
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

// --- RopyBoard hotkey recording methods ---

impl RopyBoard {
    pub(crate) fn displayed_hotkey(&self) -> &str {
        &self.pending_hotkey
    }

    pub(crate) fn start_hotkey_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hotkey_before_recording = self.pending_hotkey.clone();
        self.hotkey_recording = true;
        self.hotkey_manual_editing = false;
        self.settings_activation_key_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn enable_hotkey_manual_edit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.hotkey_recording = false;
        self.hotkey_manual_editing = true;
        let pending_hotkey = self.pending_hotkey.clone();
        self.settings_activation_key_input.update(cx, |input, cx| {
            input.set_value(pending_hotkey, window, cx);
        });
        window.focus(&self.settings_activation_key_input.focus_handle(cx));
        cx.notify();
    }

    pub(crate) fn clear_hotkey_candidate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hotkey_recording = false;
        self.pending_hotkey.clear();
        self.settings_activation_key_input.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn cancel_hotkey_recording(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.hotkey_recording = false;
        self.pending_hotkey
            .clone_from(&self.hotkey_before_recording);
        window.focus(&self.focus_handle);
        cx.notify();
    }

    pub(crate) fn on_settings_key_down(
        &mut self,
        event: &gpui::KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.hotkey_recording {
            return;
        }

        if is_cancel_key(&event.keystroke.key) {
            self.cancel_hotkey_recording(window, cx);
            return;
        }

        if is_clear_key(&event.keystroke) {
            self.clear_hotkey_candidate(window, cx);
            return;
        }

        let Some(hotkey) = keystroke_to_hotkey(&event.keystroke) else {
            return;
        };

        self.hotkey_recording = false;
        self.hotkey_manual_editing = false;
        self.pending_hotkey = hotkey;
        window.focus(&self.focus_handle);
        cx.notify();
    }
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
    fn test_keystroke_to_hotkey_modifier_only_returns_none() {
        let mut modifiers = Modifiers::none();
        modifiers.control = true;

        assert!(keystroke_to_hotkey(&keystroke("control", modifiers)).is_none());
    }

    #[test]
    fn test_is_clear_key_requires_no_modifiers() {
        let mut modifiers = Modifiers::none();
        modifiers.control = true;

        assert!(is_clear_key(&keystroke("delete", Modifiers::none())));
        assert!(!is_clear_key(&keystroke("delete", modifiers)));
    }

    #[test]
    fn test_is_cancel_key_escape_variants_returns_true() {
        assert!(is_cancel_key("escape"));
        assert!(is_cancel_key("esc"));
        assert!(!is_cancel_key("enter"));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_keystroke_to_hotkey_control_shift_letter_round_trips() {
        let mut modifiers = Modifiers::none();
        modifiers.control = true;
        modifiers.shift = true;

        let hotkey = keystroke_to_hotkey(&keystroke("d", modifiers)).unwrap();
        assert!(HotKey::from_str(&hotkey).is_ok());
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_keystroke_to_hotkey_command_shift_letter_round_trips() {
        let mut modifiers = Modifiers::none();
        modifiers.platform = true;
        modifiers.shift = true;

        let hotkey = keystroke_to_hotkey(&keystroke("v", modifiers)).unwrap();
        assert_eq!(hotkey, "cmd+shift+v");
        assert!(HotKey::from_str(&hotkey).is_ok());
    }
}

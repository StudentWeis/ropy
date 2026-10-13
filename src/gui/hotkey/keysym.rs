//! Key names shared by Linux shortcut backends.

pub(super) fn keysym(code: &str) -> Option<String> {
    if let Some(key) = code
        .strip_prefix("Key")
        .or_else(|| code.strip_prefix("Digit"))
    {
        return Some(key.to_owned());
    }
    if code
        .strip_prefix('F')
        .is_some_and(|n| n.parse::<u8>().is_ok_and(|n| (1..=24).contains(&n)))
    {
        return Some(code.to_owned());
    }
    Some(
        match code {
            "Enter" => "Return",
            "Space" => "space",
            "Tab" => "Tab",
            "Escape" => "Escape",
            "Backspace" => "BackSpace",
            "Delete" => "Delete",
            "Insert" => "Insert",
            "Home" => "Home",
            "End" => "End",
            "PageUp" => "Prior",
            "PageDown" => "Next",
            "ArrowUp" => "Up",
            "ArrowDown" => "Down",
            "ArrowLeft" => "Left",
            "ArrowRight" => "Right",
            "Minus" => "minus",
            "Equal" => "equal",
            "Comma" => "comma",
            "Period" => "period",
            "Slash" => "slash",
            "Backslash" => "backslash",
            "Semicolon" => "semicolon",
            "Quote" => "apostrophe",
            "Backquote" => "grave",
            "BracketLeft" => "bracketleft",
            "BracketRight" => "bracketright",
            _ => return None,
        }
        .to_owned(),
    )
}

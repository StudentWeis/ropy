use gpui_kit::{
    Action, App, AsKeystroke, Global, KeyBinding, KeyContext, OperatingSystem, SharedString,
};
use rust_i18n::t;
use thiserror::Error;

use super::{
    ActivePanel, ConfirmRecord, ConfirmSelection, ConfirmSelectionPlainText, CycleFilterNext,
    CycleFilterPrev, DeleteRecord, FocusSearch, Hide, PreviewSelection, QUICK_SELECT_COUNT, Quit,
    RopyBoard, SelectLeft, SelectNext, SelectPrev, SelectRight, ToggleFavorite,
    ToggleFavoritesFilter, ToggleWindowPin,
};
use crate::{
    config::{KeybindingSettings, Settings},
    gui::settings::GlobalSettings,
};

const BOARD: Option<&str> = Some("Ropy && !Select && !List");
const HISTORY: Option<&str> = Some("RopyHistory && !Input && !Select && !List");
const SEARCH: Option<&str> = Some("RopyHistory > Input && !Select && !List");

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ShortcutGroup {
    Common,
    Navigation,
    QuickSelection,
    Window,
}

pub(crate) struct HistoryShortcut {
    pub(crate) id: String,
    pub(crate) label: SharedString,
    pub(crate) group: ShortcutGroup,
    primary: KeyBinding,
    aliases: Vec<KeyBinding>,
}

impl HistoryShortcut {
    fn new(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        group: ShortcutGroup,
        action: impl Action + Clone,
        primary: &'static str,
        aliases: &[&'static str],
    ) -> Self {
        let aliases = aliases
            .iter()
            .map(|key| KeyBinding::new(key, action.clone(), HISTORY))
            .collect();
        Self {
            id: id.into(),
            label: label.into(),
            group,
            primary: KeyBinding::new(primary, action, HISTORY),
            aliases,
        }
    }

    pub(crate) fn default_keys(&self, os: OperatingSystem) -> Vec<String> {
        std::iter::once(&self.primary)
            .chain(&self.aliases)
            .map(|binding| binding.keystrokes()[0].as_keystroke().unparse(os))
            .collect()
    }
}

// This is the single source of default history bindings and their stable config IDs.
#[expect(
    clippy::too_many_lines,
    reason = "keep catalog metadata beside each default binding"
)]
pub(crate) fn history_shortcuts() -> Vec<HistoryShortcut> {
    use ShortcutGroup::{Common, Navigation, QuickSelection, Window};
    let mut shortcuts = vec![
        HistoryShortcut::new("hide", t!("help_hide"), Window, Hide, "q", &[]),
        HistoryShortcut::new(
            "focus_search",
            t!("help_search"),
            Common,
            FocusSearch,
            "/",
            &[],
        ),
        HistoryShortcut::new(
            "preview_selection",
            t!("help_toggle_preview"),
            Common,
            PreviewSelection,
            "space",
            &[],
        ),
        HistoryShortcut::new(
            "toggle_window_pin",
            t!("help_pin"),
            Window,
            ToggleWindowPin,
            "p",
            &[],
        ),
        HistoryShortcut::new(
            "toggle_favorite",
            t!("help_favorite"),
            Common,
            ToggleFavorite,
            "f",
            &[],
        ),
        HistoryShortcut::new(
            "select_left",
            t!("shortcuts_left"),
            Navigation,
            SelectLeft,
            "left",
            &["h"],
        ),
        HistoryShortcut::new(
            "select_right",
            t!("shortcuts_right"),
            Navigation,
            SelectRight,
            "right",
            &["l"],
        ),
        HistoryShortcut::new(
            "select_prev",
            t!("shortcuts_prev"),
            Navigation,
            SelectPrev,
            "up",
            &["k"],
        ),
        HistoryShortcut::new(
            "select_next",
            t!("shortcuts_next"),
            Navigation,
            SelectNext,
            "down",
            &["j"],
        ),
        HistoryShortcut::new(
            "delete_record",
            t!("help_delete"),
            Common,
            DeleteRecord,
            "delete",
            &["d"],
        ),
        HistoryShortcut::new(
            "confirm_selection",
            t!("help_confirm"),
            Common,
            ConfirmSelection,
            "enter",
            &[],
        ),
        HistoryShortcut::new(
            "confirm_selection_plain_text",
            t!("help_confirm_plain_text"),
            Common,
            ConfirmSelectionPlainText,
            "shift-enter",
            &[],
        ),
        HistoryShortcut::new(
            "cycle_filter_next",
            t!("shortcuts_filter_next"),
            Navigation,
            CycleFilterNext,
            "alt-right",
            &[],
        ),
        HistoryShortcut::new(
            "cycle_filter_prev",
            t!("shortcuts_filter_prev"),
            Navigation,
            CycleFilterPrev,
            "alt-left",
            &[],
        ),
        HistoryShortcut::new(
            "toggle_favorites_filter",
            t!("help_toggle_favorites_filter"),
            Navigation,
            ToggleFavoritesFilter,
            "alt-f",
            &[],
        ),
    ];
    let quick_keys: [&str; QUICK_SELECT_COUNT] = ["1", "2", "3", "4", "5"];
    shortcuts.extend(quick_keys.into_iter().enumerate().map(|(index, key)| {
        HistoryShortcut::new(
            format!("confirm_record_{}", index + 1),
            t!("shortcuts_quick_select", index = index + 1),
            QuickSelection,
            ConfirmRecord { index },
            key,
            &[],
        )
    }));
    shortcuts
}

fn window_bindings() -> [KeyBinding; 2] {
    let quit = cfg_select! {
        target_os = "macos" => KeyBinding::new("cmd-q", Quit, None),
        _ => KeyBinding::new("alt-f4", Quit, None),
    };
    [KeyBinding::new("escape", Hide, BOARD), quit]
}

#[derive(Debug, Error)]
pub(crate) enum KeybindingError {
    #[error("unknown history action: {action}")]
    UnknownAction { action: String },
    #[error("invalid key {key:?} for history action {action}")]
    InvalidKey { action: String, key: String },
    #[error("reserved key {key:?} for history action {action}")]
    ReservedKey { action: String, key: String },
    #[error("key {key:?} conflicts between history actions {first} and {second}")]
    Conflict {
        key: String,
        first: String,
        second: String,
    },
}

impl KeybindingError {
    pub(crate) fn affects(&self, id: &str) -> bool {
        match self {
            Self::UnknownAction { action }
            | Self::InvalidKey { action, .. }
            | Self::ReservedKey { action, .. } => action == id,
            Self::Conflict { first, second, .. } => first == id || second == id,
        }
    }

    pub(crate) fn message(&self) -> SharedString {
        let shortcuts = history_shortcuts();
        let label = |id: &str| {
            shortcuts
                .iter()
                .find(|s| s.id == id)
                .map_or_else(|| SharedString::from(id.to_owned()), |s| s.label.clone())
        };
        match self {
            Self::UnknownAction { action } => t!("keybindings_unknown_action", action = action),
            Self::InvalidKey { action, key } => {
                t!("keybindings_invalid_key", action = label(action), key = key)
            }
            Self::ReservedKey { action, key } => t!(
                "keybindings_reserved_key",
                action = label(action),
                key = key
            ),
            Self::Conflict { key, first, second } => t!(
                "keybindings_conflict",
                key = key,
                first = label(first),
                second = label(second)
            ),
        }
        .into()
    }
}

#[derive(Default)]
struct KeybindingStatus(Option<KeybindingError>);
impl Global for KeybindingStatus {}

pub(crate) fn keybinding_error_message(cx: &App) -> Option<SharedString> {
    let error = cx.try_global::<KeybindingStatus>()?.0.as_ref()?;
    let detail = match error {
        KeybindingError::UnknownAction { action } => {
            t!("keybindings_unknown_action", action = action)
        }
        KeybindingError::InvalidKey { action, key } => {
            t!("keybindings_invalid_key", action = action, key = key)
        }
        KeybindingError::ReservedKey { action, key } => {
            t!("keybindings_reserved_key", action = action, key = key)
        }
        KeybindingError::Conflict { key, first, second } => t!(
            "keybindings_conflict",
            key = key,
            first = first,
            second = second
        ),
    };
    Some(t!("keybindings_error", error = detail).into())
}

fn supported_key(key: &str) -> bool {
    key.chars().count() == 1 && !key.chars().any(char::is_control)
        || matches!(
            key,
            "space"
                | "tab"
                | "enter"
                | "escape"
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
        )
        || key
            .strip_prefix('f')
            .and_then(|n| n.parse::<u8>().ok())
            .is_some_and(|n| (1..=24).contains(&n) && key == format!("f{n}"))
}

pub(crate) fn compile_history_bindings(
    settings: &KeybindingSettings,
    cx: &App,
) -> Result<Vec<KeyBinding>, KeybindingError> {
    let shortcuts = history_shortcuts();
    for action in settings.history.keys() {
        if !shortcuts.iter().any(|shortcut| &shortcut.id == action) {
            return Err(KeybindingError::UnknownAction {
                action: action.clone(),
            });
        }
    }
    let reserved = window_bindings();
    let mut result: Vec<(String, KeyBinding)> = Vec::new();
    for shortcut in shortcuts {
        let bindings = if let Some(keys) = settings.history.get(&shortcut.id) {
            keys.iter()
                .map(|key| parse_history_binding(&shortcut, key, &reserved, cx))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            std::iter::once(shortcut.primary)
                .chain(shortcut.aliases)
                .collect()
        };
        for binding in bindings {
            if let Some((other, _)) = result
                .iter()
                .find(|(_, existing)| existing.keystrokes() == binding.keystrokes())
            {
                if other == &shortcut.id {
                    continue;
                }
                return Err(KeybindingError::Conflict {
                    key: binding
                        .keystrokes()
                        .iter()
                        .map(|key| key.as_keystroke().unparse(cx.platform_info().os))
                        .collect::<Vec<_>>()
                        .join(" "),
                    first: other.clone(),
                    second: shortcut.id,
                });
            }
            result.push((shortcut.id.clone(), binding));
        }
    }
    Ok(result.into_iter().map(|(_, binding)| binding).collect())
}

fn parse_history_binding(
    shortcut: &HistoryShortcut,
    key: &str,
    reserved: &[KeyBinding],
    cx: &App,
) -> Result<KeyBinding, KeybindingError> {
    let invalid = || KeybindingError::InvalidKey {
        action: shortcut.id.clone(),
        key: key.into(),
    };
    let binding = KeyBinding::load(
        key,
        shortcut.primary.action().boxed_clone(),
        shortcut.primary.predicate(),
        false,
        None,
        cx,
    )
    .map_err(|_| invalid())?;
    let [stroke] = binding.keystrokes() else {
        return Err(invalid());
    };
    let stroke = stroke.as_keystroke();
    if !supported_key(&stroke.key) || stroke.key_char.is_some() {
        return Err(invalid());
    }
    if stroke.key == "tab"
        || reserved
            .iter()
            .any(|reserved| reserved.keystrokes() == binding.keystrokes())
    {
        return Err(KeybindingError::ReservedKey {
            action: shortcut.id.clone(),
            key: key.into(),
        });
    }
    Ok(binding)
}

/// Register once at startup, after settings and component bindings are loaded.
pub(crate) fn bind_application_keys(cx: &mut App) {
    cx.bind_keys(window_bindings());
    // These keys retain their search-input behavior regardless of history overrides.
    cx.bind_keys([
        KeyBinding::new("up", SelectPrev, SEARCH),
        KeyBinding::new("down", SelectNext, SEARCH),
        KeyBinding::new("enter", ConfirmSelection, SEARCH),
        KeyBinding::new("shift-enter", ConfirmSelectionPlainText, SEARCH),
        KeyBinding::new("alt-right", CycleFilterNext, SEARCH),
        KeyBinding::new("alt-left", CycleFilterPrev, SEARCH),
        KeyBinding::new("alt-f", ToggleFavoritesFilter, SEARCH),
    ]);
    cx.set_global(KeybindingStatus::default());
    if GlobalSettings::read(cx, Settings::is_recovery_required) {
        return;
    }
    match GlobalSettings::read(cx, |settings| {
        compile_history_bindings(&settings.keybindings, cx)
    }) {
        Ok(bindings) => cx.bind_keys(bindings),
        Err(error) => {
            tracing::warn!(%error, "history shortcuts disabled; repair config.toml and restart");
            cx.set_global(KeybindingStatus(Some(error)));
        }
    }
}

/// Replace only bindings owned by the history catalog, preserving bindings added
/// by components after startup as well as fixed search and window controls.
pub(crate) fn replace_history_bindings(bindings: Vec<KeyBinding>, cx: &mut App) {
    let shortcuts = history_shortcuts();
    let retained: Vec<_> = cx
        .key_bindings()
        .borrow()
        .bindings()
        .filter(|binding| {
            !shortcuts.iter().any(|shortcut| {
                binding.predicate() == shortcut.primary.predicate()
                    && binding.action().partial_eq(shortcut.primary.action())
            })
        })
        .cloned()
        .collect();
    cx.clear_key_bindings();
    cx.bind_keys(retained);
    cx.bind_keys(bindings);
    cx.set_global(KeybindingStatus::default());
}

fn board_context(history_active: bool) -> KeyContext {
    let mut context = KeyContext::default();
    context.add("Ropy");
    if history_active {
        context.add("RopyHistory");
    }
    context
}

pub(crate) fn binding_applies_to_history(binding: &KeyBinding) -> bool {
    binding
        .predicate()
        .is_none_or(|predicate| predicate.eval(&[board_context(true)]))
}

impl RopyBoard {
    pub(crate) fn shortcut_context(&self) -> KeyContext {
        board_context(
            self.activated
                && self.active_panel == ActivePanel::ClipboardList
                && !self.ui_state.any_overlay_visible(),
        )
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use rstest::rstest;

    use super::*;

    fn compile(settings: &KeybindingSettings) -> Result<Vec<KeyBinding>, KeybindingError> {
        gpui_kit::TestAppContext::single().read(|cx| compile_history_bindings(settings, cx))
    }

    #[rstest]
    #[case(OperatingSystem::Mac, "cmd-d")]
    #[case(OperatingSystem::Windows, "ctrl-d")]
    #[case(OperatingSystem::Linux, "ctrl-d")]
    fn test_history_secondary_modifier_uses_runtime_host(
        #[case] os: OperatingSystem,
        #[case] expected: &str,
    ) {
        let mut cx = gpui_kit::TestAppContext::single();
        cx.set_platform_info(gpui_kit::PlatformInfo {
            os,
            target: gpui_kit::PlatformTarget::Native,
        });
        cx.read(|cx| {
            let settings = overrides("delete_record", &["secondary-d"]);
            let bindings = compile_history_bindings(&settings, cx).unwrap();
            let binding = bindings
                .iter()
                .find(|binding| binding.action().as_any().is::<DeleteRecord>())
                .unwrap();
            assert_eq!(binding.keystrokes()[0].as_keystroke().unparse(os), expected);
            let mut settings = settings;
            settings
                .history
                .insert("toggle_favorite".into(), vec![expected.into()]);
            assert!(matches!(
                compile_history_bindings(&settings, cx),
                Err(KeybindingError::Conflict { .. })
            ));
        });
    }

    fn overrides(action: &str, keys: &[&str]) -> KeybindingSettings {
        let mut settings = KeybindingSettings::default();
        settings.history.insert(
            action.into(),
            keys.iter().map(|key| (*key).into()).collect(),
        );
        settings
    }

    #[rstest]
    #[case("")]
    #[case("ctrl")]
    #[case("ctrl-k ctrl-d")]
    #[case("banana")]
    #[case("ctrl+delete")]
    #[case("f25")]
    #[case("f01")]
    #[case("madeup-d")]
    fn test_history_invalid_key_returns_error(#[case] key: &str) {
        assert!(
            matches!(
                compile(&overrides("delete_record", &[key])),
                Err(KeybindingError::InvalidKey { .. })
            ),
            "{key}"
        );
    }

    #[rstest]
    #[case("escape")]
    #[case("tab")]
    #[case("shift-tab")]
    fn test_history_reserved_key_returns_error(#[case] key: &str) {
        assert!(matches!(
            compile(&overrides("delete_record", &[key])),
            Err(KeybindingError::ReservedKey { .. })
        ));
    }

    #[test]
    fn test_history_platform_quit_key_is_reserved() {
        let key = if cfg!(target_os = "macos") {
            "cmd-q"
        } else {
            "alt-f4"
        };
        assert!(matches!(
            compile(&overrides("delete_record", &[key])),
            Err(KeybindingError::ReservedKey { .. })
        ));
    }

    #[test]
    fn test_history_unknown_action_returns_error_even_when_disabled() {
        assert!(matches!(
            compile(&overrides("delete_reocrd", &[])),
            Err(KeybindingError::UnknownAction { .. })
        ));
    }

    #[rstest]
    #[case("ctrl-d", "CTRL-d")]
    #[case("D", "shift-d")]
    fn test_history_normalized_keys_detect_conflict(#[case] first: &str, #[case] second: &str) {
        let mut settings = overrides("delete_record", &[first]);
        settings
            .history
            .insert("toggle_favorite".into(), vec![second.into()]);
        assert!(matches!(
            compile(&settings),
            Err(KeybindingError::Conflict { .. })
        ));
    }

    #[test]
    fn test_history_disabling_action_releases_keys_and_removing_override_restores_defaults() {
        let mut settings = overrides("delete_record", &["f", "f"]);
        assert!(matches!(
            compile(&settings),
            Err(KeybindingError::Conflict { .. })
        ));
        settings.history.insert("toggle_favorite".into(), vec![]);
        let bindings = compile(&settings).unwrap();
        let delete = bindings
            .iter()
            .filter(|binding| binding.action().as_any().is::<DeleteRecord>())
            .collect::<Vec<_>>();
        assert_eq!(delete.len(), 1);
        assert_eq!(
            delete[0].keystrokes()[0]
                .as_keystroke()
                .unparse(OperatingSystem::Linux),
            "f"
        );
        assert!(
            !bindings
                .iter()
                .any(|binding| binding.action().as_any().is::<ToggleFavorite>())
        );

        settings.history.clear();
        let bindings = compile(&settings).unwrap();
        let delete = bindings
            .iter()
            .filter(|binding| binding.action().as_any().is::<DeleteRecord>())
            .map(|binding| {
                binding.keystrokes()[0]
                    .as_keystroke()
                    .unparse(OperatingSystem::Linux)
            })
            .collect::<Vec<_>>();
        assert_eq!(delete, ["delete", "d"]);
        assert!(
            bindings
                .iter()
                .any(|binding| binding.action().as_any().is::<ToggleFavorite>())
        );
    }
}

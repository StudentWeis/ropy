use gpui_kit::{App, KeyBinding, KeyContext};

use super::{
    ActivePanel, ConfirmRecord, ConfirmSelection, ConfirmSelectionPlainText, CycleFilterNext,
    CycleFilterPrev, DeleteRecord, FocusSearch, Hide, PreviewSelection, QUICK_SELECT_COUNT, Quit,
    RopyBoard, SelectLeft, SelectNext, SelectPrev, SelectRight, ToggleFavorite,
    ToggleFavoritesFilter, ToggleWindowPin,
};

// Selectors and popup lists own navigation, confirmation and dismissal. Search
// inputs own characters, cursor movement and deletion, but allow result navigation.
const BOARD: Option<&str> = Some("Ropy && !Select && !List");
const HISTORY: Option<&str> = Some("RopyHistory && !Select && !List");
const HISTORY_WITHOUT_INPUT: Option<&str> = Some("RopyHistory && !Input && !Select && !List");

#[expect(
    clippy::redundant_pub_crate,
    reason = "re-exported at crate visibility by actions"
)]
pub(crate) fn bind_application_keys(cx: &mut App) {
    let quit_key_binding = cfg_select! {
        target_os = "macos" => KeyBinding::new("cmd-q", Quit, None),
        _ => KeyBinding::new("alt-f4", Quit, None),
    };

    cx.bind_keys([
        KeyBinding::new("escape", Hide, BOARD),
        quit_key_binding,
        KeyBinding::new("q", Hide, HISTORY_WITHOUT_INPUT),
        KeyBinding::new("/", FocusSearch, HISTORY_WITHOUT_INPUT),
        KeyBinding::new("space", PreviewSelection, HISTORY_WITHOUT_INPUT),
        KeyBinding::new("p", ToggleWindowPin, HISTORY_WITHOUT_INPUT),
        KeyBinding::new("f", ToggleFavorite, HISTORY_WITHOUT_INPUT),
        KeyBinding::new("left", SelectLeft, HISTORY_WITHOUT_INPUT),
        KeyBinding::new("h", SelectLeft, HISTORY_WITHOUT_INPUT),
        KeyBinding::new("right", SelectRight, HISTORY_WITHOUT_INPUT),
        KeyBinding::new("l", SelectRight, HISTORY_WITHOUT_INPUT),
        KeyBinding::new("up", SelectPrev, HISTORY),
        KeyBinding::new("k", SelectPrev, HISTORY_WITHOUT_INPUT),
        KeyBinding::new("down", SelectNext, HISTORY),
        KeyBinding::new("j", SelectNext, HISTORY_WITHOUT_INPUT),
        KeyBinding::new("delete", DeleteRecord, HISTORY_WITHOUT_INPUT),
        KeyBinding::new("d", DeleteRecord, HISTORY_WITHOUT_INPUT),
        KeyBinding::new("enter", ConfirmSelection, HISTORY),
        KeyBinding::new("shift-enter", ConfirmSelectionPlainText, HISTORY),
        KeyBinding::new("alt-right", CycleFilterNext, HISTORY),
        KeyBinding::new("alt-left", CycleFilterPrev, HISTORY),
        KeyBinding::new("alt-f", ToggleFavoritesFilter, HISTORY),
    ]);
    cx.bind_keys((0..QUICK_SELECT_COUNT).map(|index| {
        KeyBinding::new(
            &(index + 1).to_string(),
            ConfirmRecord { index },
            HISTORY_WITHOUT_INPUT,
        )
    }));
}

impl RopyBoard {
    pub(crate) fn shortcut_context(&self) -> KeyContext {
        let mut context = KeyContext::default();
        context.add("Ropy");
        if self.activated
            && self.active_panel == ActivePanel::ClipboardList
            && !self.ui_state.any_overlay_visible()
        {
            context.add("RopyHistory");
        }
        context
    }
}

use gpui_kit::{
    App, Context, FocusHandle, ScrollHandle, SharedString, Subscription, Window,
    component::{WindowExt, button::ButtonVariant, dialog::DialogButtonProps},
};
use rust_i18n::t;

use super::{
    ActivePanel, RopyBoard,
    actions::shortcuts::{
        HistoryShortcut, KeybindingError, compile_history_bindings, replace_history_bindings,
    },
};
use crate::{config::KeybindingSettings, gui::settings::GlobalSettings};

pub(crate) struct Recording {
    pub(crate) action: String,
    previous: Option<String>,
}

pub(crate) struct ShortcutEditor {
    pub(crate) scroll: ScrollHandle,
    pub(crate) row_focus: std::collections::HashMap<String, FocusHandle>,
    baseline: KeybindingSettings,
    pub(crate) draft: KeybindingSettings,
    pub(crate) error: Option<KeybindingError>,
    pub(crate) save_error: Option<SharedString>,
    pub(crate) recording: Option<Recording>,
    return_panel: ActivePanel,
}

impl ShortcutEditor {
    fn new(settings: KeybindingSettings, return_panel: ActivePanel, cx: &App) -> Self {
        let error = compile_history_bindings(&settings).err();
        let scroll = ScrollHandle::new();
        let row_focus = super::actions::shortcuts::history_shortcuts()
            .into_iter()
            .map(|shortcut| (shortcut.id, cx.focus_handle()))
            .collect();
        Self {
            scroll,
            row_focus,
            baseline: settings.clone(),
            draft: settings,
            error,
            save_error: None,
            recording: None,
            return_panel,
        }
    }

    pub(crate) fn is_dirty(&self) -> bool {
        self.draft != self.baseline
    }

    pub(crate) fn keys(&self, shortcut: &HistoryShortcut) -> Vec<String> {
        let mut keys = self
            .draft
            .history
            .get(&shortcut.id)
            .cloned()
            .unwrap_or_else(|| shortcut.default_keys());
        // A manually edited file may repeat a key; show one removable control.
        let mut seen = std::collections::HashSet::new();
        keys.retain(|key| seen.insert(key.clone()));
        keys
    }

    pub(crate) fn is_modified(&self, shortcut: &HistoryShortcut) -> bool {
        let normalize = |keys: Vec<String>| -> std::collections::HashSet<String> {
            keys.into_iter()
                .map(|key| gpui_kit::Keystroke::parse(&key).map_or(key, |key| key.unparse()))
                .collect()
        };
        normalize(self.keys(shortcut)) != normalize(shortcut.default_keys())
    }

    pub(crate) fn edit(&mut self, edit: impl FnOnce(&mut KeybindingSettings)) {
        self.recording = None;
        self.save_error = None;
        edit(&mut self.draft);
        self.error = compile_history_bindings(&self.draft).err();
    }

    pub(crate) fn revert(&mut self) {
        let baseline = self.baseline.clone();
        self.edit(|draft| *draft = baseline);
    }
}

impl RopyBoard {
    pub(crate) fn open_shortcuts(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let settings = GlobalSettings::read(cx, |s| s.keybindings.clone());
        if let Some(editor) = &mut self.shortcut_editor
            && editor.is_dirty()
        {
            // A tray navigation may temporarily show another panel. Keep its draft.
            editor.return_panel = self.active_panel;
            editor.recording = None;
        } else {
            self.shortcut_editor = Some(ShortcutEditor::new(settings, self.active_panel, cx));
        }
        self.active_panel = ActivePanel::Shortcuts;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    pub(crate) fn edit_shortcuts(
        &mut self,
        edit: impl FnOnce(&mut ShortcutEditor),
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(editor) = &mut self.shortcut_editor {
            edit(editor);
            cx.notify();
        }
    }

    pub(crate) fn record_shortcut(
        &mut self,
        action: String,
        previous: Option<String>,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if let Some(editor) = &mut self.shortcut_editor {
            editor.recording = Some(Recording { action, previous });
            window.focus(&self.focus_handle, cx);
            cx.notify();
        }
    }

    pub(crate) fn cancel_shortcut_recording(&mut self, cx: &mut Context<'_, Self>) {
        if let Some(editor) = &mut self.shortcut_editor
            && editor.recording.take().is_some()
        {
            cx.notify();
        }
    }

    pub(super) fn observe_shortcut_recording(cx: &mut Context<'_, Self>) -> Subscription {
        let owner = cx.entity().downgrade();
        cx.intercept_keystrokes(move |event, window, cx| {
            let _ = owner.update(cx, |board, cx| {
                if board.active_panel != ActivePanel::Shortcuts
                    || !board.focus_handle.contains_focused(window, cx)
                {
                    return;
                }
                let Some(editor) = &mut board.shortcut_editor else {
                    return;
                };
                let Some(recording) = editor.recording.take() else {
                    return;
                };
                // Tab continues normal focus traversal. Escape only cancels recording.
                if event.keystroke.key != "tab" {
                    cx.stop_propagation();
                }
                if !matches!(event.keystroke.key.as_str(), "escape" | "tab") {
                    let shortcut = super::actions::shortcuts::history_shortcuts()
                        .into_iter()
                        .find(|s| s.id == recording.action);
                    if let Some(shortcut) = shortcut {
                        let mut keys = editor.keys(&shortcut);
                        let key = event.keystroke.unparse();
                        if let Some(index) = recording
                            .previous
                            .as_ref()
                            .and_then(|old| keys.iter().position(|key| key == old))
                        {
                            keys[index] = key;
                        } else {
                            keys.push(key);
                        }
                        editor.edit(|draft| {
                            draft.history.insert(recording.action, keys);
                        });
                    }
                }
                cx.notify();
            });
        })
    }

    pub(crate) fn apply_shortcuts(&mut self, cx: &mut Context<'_, Self>) {
        let Some(editor) = &mut self.shortcut_editor else {
            return;
        };
        if editor.recording.is_some() || !editor.is_dirty() {
            return;
        }
        let bindings = match compile_history_bindings(&editor.draft) {
            Ok(bindings) => bindings,
            Err(error) => {
                editor.error = Some(error);
                cx.notify();
                return;
            }
        };
        match GlobalSettings::persist(cx, |s| s.keybindings = editor.draft.clone()) {
            Ok(()) => {
                replace_history_bindings(bindings, cx);
                editor.baseline = editor.draft.clone();
                editor.error = None;
                editor.save_error = None;
            }
            Err(error) => {
                tracing::warn!(%error, "failed to save history shortcuts");
                editor.save_error = Some(t!("settings_save_failed", error = error).into());
            }
        }
        cx.notify();
    }

    fn close_shortcuts(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(editor) = self.shortcut_editor.take() {
            self.active_panel = editor.return_panel;
            if self.active_panel == ActivePanel::Settings {
                self.open_settings_panel(window, cx);
            } else {
                window.focus(&self.focus_handle, cx);
            }
            cx.notify();
        }
    }

    pub(crate) fn leave_shortcuts(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        self.cancel_shortcut_recording(cx);
        if !self
            .shortcut_editor
            .as_ref()
            .is_some_and(ShortcutEditor::is_dirty)
        {
            self.close_shortcuts(window, cx);
            return;
        }
        let owner = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |dialog, _, _| {
            let owner = owner.clone();
            dialog
                .confirm()
                .title(t!("shortcuts_discard_title"))
                .description(t!("shortcuts_discard_description"))
                .button_props(
                    DialogButtonProps::default()
                        .ok_variant(ButtonVariant::Danger)
                        .ok_text(t!("shortcuts_discard"))
                        .cancel_text(t!("shortcuts_continue")),
                )
                .on_ok(move |_, window, cx: &mut App| {
                    let _ = owner.update(cx, |board, cx| board.close_shortcuts(window, cx));
                    true
                })
        });
    }
}

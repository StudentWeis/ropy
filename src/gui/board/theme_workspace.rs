//! Window-bound draft controls and application commands.
mod color;
use super::{ActivePanel, RopyBoard};
use crate::gui::theme::{
    document::{ColorRole, DocumentError, ThemeDocument},
    storage::{ThemeWrite, ThemeWriteError},
};
use crate::gui::{
    settings::GlobalSettings,
    theme::{ThemeCatalog, ThemeDefinition, ThemeId, ThemeMode},
};
use color::ColorEditor;
use gpui_kit::{
    App, AppContext, Context, Entity, Subscription, Window,
    component::{WindowExt, button::ButtonVariant, dialog::DialogButtonProps, input::InputState},
};
use rust_i18n::t;

#[derive(Default)]
pub(crate) struct ThemeFileState {
    pub(crate) busy: bool,
    pub(crate) message: Option<String>,
}

pub(crate) struct ThemeWorkspace {
    pub(crate) selected: ThemeId,
    pub(crate) draft: Option<ThemeDraft>,
    pub(crate) error: Option<String>,
}

pub(crate) struct ThemeDraft {
    pub(crate) name: Entity<InputState>,
    pub(crate) dark: bool,
    pub(crate) advanced: bool,
    pub(crate) preview_role: ColorRole,
    pub(crate) colors: Vec<ThemeColor>,
    baseline: ThemeDocument,
    target: Option<(ThemeId, String)>,
    subscriptions: Vec<Subscription>,
}

pub(crate) struct ThemeColor {
    pub(crate) role: ColorRole,
    pub(crate) editor: Entity<ColorEditor>,
}

impl ThemeDraft {
    pub(super) fn attach_window(&mut self, window: &mut Window, cx: &mut Context<'_, RopyBoard>) {
        let name = self.name.read(cx).value();
        self.name = cx.new(|cx| InputState::new(window, cx).default_value(name));
        self.subscriptions = vec![cx.observe(&self.name, |_, _, cx| cx.notify())];
        for color in &mut self.colors {
            let value = color
                .editor
                .read(cx)
                .value(cx)
                .unwrap_or_else(|| color.editor.read(cx).preview());
            let text = color.editor.read(cx).input.read(cx).value().to_string();
            color.editor = cx.new(|cx| ColorEditor::new(color.role, value, Some(text), window, cx));
            self.subscriptions
                .push(observe_color(&color.editor, color.role, cx));
        }
    }

    fn new(
        theme: &ThemeDefinition,
        source: String,
        copy: bool,
        window: &mut Window,
        cx: &mut Context<'_, RopyBoard>,
    ) -> Self {
        let mut baseline = ThemeDocument::from(theme);
        if copy {
            baseline.name = t!("themes_copy_name", name = theme.name()).to_string();
        }
        let name = cx.new(|cx| InputState::new(window, cx).default_value(baseline.name.clone()));
        let colors = ColorRole::ALL
            .iter()
            .map(|&role| ThemeColor {
                role,
                editor: cx.new(|cx| {
                    ColorEditor::new(role, role.get(&baseline.palette), None, window, cx)
                }),
            })
            .collect::<Vec<_>>();
        let mut subscriptions = vec![cx.observe(&name, |_, _, cx| cx.notify())];
        subscriptions.extend(
            colors
                .iter()
                .map(|color| observe_color(&color.editor, color.role, cx)),
        );
        Self {
            name,
            dark: baseline.mode == ThemeMode::Dark,
            advanced: false,
            preview_role: ColorRole::Background,
            colors,
            baseline,
            target: (!copy && theme.id().code().starts_with("user:"))
                .then(|| (theme.id().clone(), source)),
            subscriptions,
        }
    }

    pub(crate) fn preview(&self, cx: &App) -> ThemeDocument {
        let mut document = self.baseline.clone();
        document.name = self.name.read(cx).value().to_string();
        document.mode = if self.dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        };
        for color in &self.colors {
            color
                .role
                .set(&mut document.palette, color.editor.read(cx).preview());
        }
        document
    }

    fn document(&self, cx: &App) -> Result<ThemeDocument, String> {
        let mut document = self.preview(cx);
        for color in &self.colors {
            let value = color.editor.read(cx).value(cx).ok_or_else(|| {
                t!("themes_invalid_rgb", color = t!(color.role.label_key())).to_string()
            })?;
            color.role.set(&mut document.palette, value);
        }
        Ok(document)
    }

    fn is_dirty(&self, cx: &App) -> bool {
        self.document(cx).as_ref() != Ok(&self.baseline)
    }
}

impl RopyBoard {
    pub(crate) fn open_themes(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.theme_workspace.is_none() {
            let selected = GlobalSettings::read(cx, |settings| {
                ThemeCatalog::global(cx).resolve(settings, window.appearance().into())
            });
            self.theme_workspace = Some(ThemeWorkspace {
                selected,
                draft: None,
                error: None,
            });
        }
        self.active_panel = ActivePanel::Themes;
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    pub(crate) fn new_theme_draft(
        &mut self,
        copy: bool,
        window: &mut Window,
        cx: &mut Context<'_, Self>,
    ) {
        if self.theme_files.busy {
            return;
        }
        let Some(workspace) = &mut self.theme_workspace else {
            return;
        };
        let catalog = ThemeCatalog::global(cx);
        let Some(theme) = catalog.get(&workspace.selected).cloned() else {
            return;
        };
        let Some(source) = catalog.source(&workspace.selected).map(str::to_owned) else {
            return;
        };
        workspace.draft = Some(ThemeDraft::new(&theme, source, copy, window, cx));
        workspace.error = None;
        cx.notify();
    }

    pub(crate) fn cancel_theme_draft(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if let Some(workspace) = &mut self.theme_workspace {
            workspace.draft = None;
            workspace.error = None;
        }
        window.focus(&self.focus_handle, cx);
        cx.notify();
    }

    pub(crate) fn leave_themes(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        let draft = self
            .theme_workspace
            .as_ref()
            .and_then(|workspace| workspace.draft.as_ref());
        if draft.is_some_and(|draft| draft.is_dirty(cx)) {
            let owner = cx.entity().downgrade();
            window.open_alert_dialog(cx, move |dialog, _, _| {
                let owner = owner.clone();
                dialog
                    .confirm()
                    .title(t!("themes_discard_title"))
                    .description(t!("themes_discard_description"))
                    .button_props(
                        DialogButtonProps::default()
                            .ok_variant(ButtonVariant::Danger)
                            .ok_text(t!("themes_discard"))
                            .cancel_text(t!("themes_continue")),
                    )
                    .on_ok(move |_, window, cx| {
                        let _ = owner.update(cx, |board, cx| board.cancel_theme_draft(window, cx));
                        true
                    })
            });
        } else if draft.is_some() {
            self.cancel_theme_draft(window, cx);
        } else {
            self.theme_workspace = None;
            self.open_settings_panel(window, cx);
        }
    }

    pub(crate) fn use_selected_theme(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.theme_files.busy {
            return;
        }
        let Some(workspace) = &mut self.theme_workspace else {
            return;
        };
        let id = workspace.selected.clone();
        match GlobalSettings::persist(cx, |settings| {
            settings.theme = id;
            settings.appearance.follow_system = false;
        }) {
            Ok(()) => {
                self.settings_editor.theme.sync(window, cx);
                crate::gui::app::apply_current_theme(
                    window,
                    cx,
                    self.settings_editor.window_opacity_percent,
                );
                workspace.error = None;
            }
            Err(error) => {
                workspace.error = Some(t!("settings_save_failed", error = error).to_string());
            }
        }
        cx.notify();
    }

    pub(crate) fn save_theme_draft(&mut self, window: &mut Window, cx: &mut Context<'_, Self>) {
        if self.theme_files.busy {
            return;
        }
        let Some(workspace) = &mut self.theme_workspace else {
            return;
        };
        let Some(draft) = &workspace.draft else {
            return;
        };
        let result = draft.document(cx).and_then(|document| {
            let directory = GlobalSettings::themes_directory(cx)
                .ok_or_else(|| t!("themes_location_failed").to_string())?;
            let write = ThemeWrite::save(
                &directory,
                &document,
                draft
                    .target
                    .as_ref()
                    .map(|(id, source)| (id, source.as_str())),
            )
            .map_err(theme_write_message)?;
            if let Err(error) = GlobalSettings::persist(cx, |settings| {
                settings.theme = write.definition.id().clone();
                settings.appearance.follow_system = false;
            }) {
                write.rollback().map_err(theme_write_message)?;
                return Err(t!("settings_save_failed", error = error).to_string());
            }
            Ok((write.definition, write.source))
        });
        match result {
            Ok((definition, source)) => {
                workspace.selected = definition.id().clone();
                workspace.draft = None;
                workspace.error = None;
                let mut catalog = ThemeCatalog::global(cx).clone();
                catalog.insert(definition, source);
                cx.set_global(catalog);
                crate::gui::app::apply_current_theme(
                    window,
                    cx,
                    self.settings_editor.window_opacity_percent,
                );
                window.focus(&self.focus_handle, cx);
            }
            Err(error) => workspace.error = Some(error),
        }
        cx.notify();
    }
}

fn theme_write_message(error: ThemeWriteError) -> String {
    tracing::warn!(%error, "theme write failed");
    match error {
        ThemeWriteError::Document(DocumentError::EmptyName) => t!("themes_empty_name").to_string(),
        ThemeWriteError::Changed => t!("themes_external_change").to_string(),
        ThemeWriteError::Restore { backup, .. } => {
            t!("themes_recovery_copy", path = backup.display()).to_string()
        }
        _ => t!("themes_file_command_failed").to_string(),
    }
}

fn observe_color(
    editor: &Entity<ColorEditor>,
    role: ColorRole,
    cx: &mut Context<'_, RopyBoard>,
) -> Subscription {
    cx.subscribe(editor, move |board, _, _: &color::ColorEdited, cx| {
        if let Some(draft) = board
            .theme_workspace
            .as_mut()
            .and_then(|workspace| workspace.draft.as_mut())
        {
            draft.preview_role = role;
        }
        cx.notify();
    })
}

//! Isolated theme drafts: preview never changes the live application palette.
use super::{ActivePanel, RopyBoard};
use crate::gui::{
    settings::GlobalSettings,
    theme::{ThemeCatalog, ThemeDefinition, ThemeId, ThemeMode},
};
use gpui_kit::{
    App, AppContext, Context, Entity, Subscription, Window,
    component::{
        WindowExt, button::ButtonVariant, color_picker::ColorPickerState,
        dialog::DialogButtonProps, input::InputState,
    },
};
use rust_i18n::t;

pub(crate) struct ThemeWorkspace {
    pub(crate) selected: ThemeId,
    pub(crate) draft: Option<ThemeDraft>,
    pub(crate) error: Option<String>,
}

pub(crate) struct ThemeDraft {
    pub(crate) name: Entity<InputState>,
    pub(crate) dark: bool,
    pub(crate) advanced: bool,
    pub(crate) colors: Vec<ThemeColor>,
    source: String,
    baseline: String,
    target: Option<ThemeId>,
    subscriptions: Vec<Subscription>,
}

pub(crate) struct ThemeColor {
    pub(crate) field: &'static str,
    pub(crate) label_key: &'static str,
    pub(crate) picker: Entity<ColorPickerState>,
}

impl ThemeDraft {
    pub(super) fn attach_window(&mut self, window: &mut Window, cx: &mut Context<'_, RopyBoard>) {
        let name = self.name.read(cx).value();
        self.name = cx.new(|cx| InputState::new(window, cx).default_value(name));
        self.subscriptions = vec![cx.observe(&self.name, |_, _, cx| cx.notify())];
        for color in &mut self.colors {
            let value = color.picker.read(cx).value().unwrap_or_default();
            color.picker = cx.new(|cx| ColorPickerState::new(window, cx).default_value(value));
            self.subscriptions
                .push(cx.observe(&color.picker, |_, _, cx| cx.notify()));
        }
    }

    fn new(
        theme: &ThemeDefinition,
        source: String,
        copy: bool,
        window: &mut Window,
        cx: &mut Context<'_, RopyBoard>,
    ) -> Self {
        let name = cx.new(|cx| {
            InputState::new(window, cx).default_value(if copy {
                t!("themes_copy_name", name = theme.name()).to_string()
            } else {
                theme.name().to_owned()
            })
        });
        let colors = color_fields(theme)
            .into_iter()
            .map(|spec| ThemeColor {
                field: spec.field,
                label_key: spec.label_key,
                picker: cx.new(|cx| {
                    ColorPickerState::new(window, cx).default_value(gpui_kit::rgb(spec.value))
                }),
            })
            .collect::<Vec<_>>();
        let mut subscriptions = vec![cx.observe(&name, |_, _, cx| cx.notify())];
        subscriptions.extend(
            colors
                .iter()
                .map(|color| cx.observe(&color.picker, |_, _, cx| cx.notify())),
        );
        let mut draft = Self {
            name,
            dark: theme.mode() == ThemeMode::Dark,
            advanced: false,
            colors,
            source,
            baseline: String::new(),
            target: (!copy && theme.id().code().starts_with("user:")).then(|| theme.id().clone()),
            subscriptions,
        };
        draft.baseline = draft.source(cx).unwrap_or_default();
        draft
    }

    pub(crate) fn color(&self, field: &str, cx: &App) -> gpui_kit::Hsla {
        self.colors
            .iter()
            .find(|color| color.field == field)
            .and_then(|color| color.picker.read(cx).displayed_color())
            .unwrap_or_default()
    }

    fn source(&self, cx: &App) -> Result<String, String> {
        let name = self.name.read(cx).value();
        if name.trim().is_empty() {
            return Err(t!("themes_empty_name").to_string());
        }
        let mut table: toml::Table =
            toml::from_str(&self.source).map_err(|error| error.to_string())?;
        table.insert("theme_name".into(), name.trim().into());
        table.insert(
            "mode".into(),
            if self.dark { "dark" } else { "light" }.into(),
        );
        for color in &self.colors {
            let value = color.picker.read(cx).value().unwrap_or_default();
            if value.a < 1.0 {
                return Err(t!("themes_opaque_only").to_string());
            }
            table.insert(color.field.into(), hex_color(value).into());
        }
        toml::to_string_pretty(&table).map_err(|error| error.to_string())
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
        if draft.is_some_and(|draft| draft.source(cx).as_ref() != Ok(&draft.baseline)) {
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
        let Some(workspace) = &mut self.theme_workspace else {
            return;
        };
        let Some(draft) = &workspace.draft else {
            return;
        };
        match save_draft(draft, cx) {
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

fn save_draft(draft: &ThemeDraft, cx: &mut App) -> Result<(ThemeDefinition, String), String> {
    use std::io::Write;
    let source = draft.source(cx)?;
    let directory = GlobalSettings::themes_directory(cx)
        .ok_or_else(|| t!("themes_location_failed").to_string())?;
    let operation = || -> Result<_, String> {
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let mut file = tempfile::Builder::new()
            .prefix("custom-")
            .suffix(".toml")
            .rand_bytes(12)
            .tempfile_in(&directory)
            .map_err(|error| error.to_string())?;
        // Generated filenames must obey the loader's lowercase filename policy.
        let stem = file
            .path()
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        let id = draft
            .target
            .clone()
            .unwrap_or_else(|| ThemeId::new(format!("user:{stem}")));
        let path = directory.join(format!("{}.toml", id.code().trim_start_matches("user:")));
        let previous = match std::fs::read(&path) {
            Ok(bytes) => Some(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.to_string()),
        };
        if draft.target.is_some() && previous.as_deref() != Some(draft.source.as_bytes()) {
            return Err(t!("themes_external_change").to_string());
        }
        let definition = ThemeDefinition::parse(&id, &source).map_err(|error| error.to_string())?;
        file.write_all(source.as_bytes())
            .map_err(|error| error.to_string())?;
        file.as_file()
            .sync_all()
            .map_err(|error| error.to_string())?;
        if draft.target.is_some() {
            file.persist(&path).map_err(|error| error.to_string())?;
        } else {
            file.persist_noclobber(&path)
                .map_err(|error| error.to_string())?;
        }
        Ok((definition, path, previous))
    };
    let (definition, path, previous) = operation()?;
    if let Err(error) = GlobalSettings::persist(cx, |settings| {
        settings.theme = definition.id().clone();
        settings.appearance.follow_system = false;
    }) {
        let rollback = previous.map_or_else(
            || std::fs::remove_file(&path),
            |bytes| std::fs::write(&path, bytes),
        );
        if let Err(rollback) = rollback {
            return Err(
                t!("themes_rollback_failed", error = error, rollback = rollback).to_string(),
            );
        }
        return Err(t!("settings_save_failed", error = error).to_string());
    }
    Ok((definition, source))
}

fn color_fields(theme: &ThemeDefinition) -> Vec<ColorSpec> {
    let palette = theme.palette();
    let mut colors = vec![
        ColorSpec {
            field: "background",
            label_key: "theme_color_background",
            value: palette.background,
        },
        ColorSpec {
            field: "foreground",
            label_key: "theme_color_foreground",
            value: palette.foreground,
        },
        ColorSpec {
            field: "primary",
            label_key: "theme_color_primary",
            value: palette.primary,
        },
        ColorSpec {
            field: "primary_foreground",
            label_key: "theme_color_primary_foreground",
            value: palette.primary_foreground,
        },
        ColorSpec {
            field: "secondary",
            label_key: "theme_color_secondary",
            value: palette.secondary,
        },
        ColorSpec {
            field: "border",
            label_key: "theme_color_border",
            value: palette.border,
        },
    ];
    colors.extend(advanced_color_fields(theme));
    colors
}

fn advanced_color_fields(theme: &ThemeDefinition) -> Vec<ColorSpec> {
    let palette = theme.palette();
    vec![
        ColorSpec {
            field: "accent",
            label_key: "theme_color_accent",
            value: palette.accent,
        },
        ColorSpec {
            field: "accent_foreground",
            label_key: "theme_color_accent_foreground",
            value: palette.accent_foreground,
        },
        ColorSpec {
            field: "secondary_foreground",
            label_key: "theme_color_secondary_foreground",
            value: palette.secondary_foreground,
        },
        ColorSpec {
            field: "muted",
            label_key: "theme_color_muted",
            value: palette.muted,
        },
        ColorSpec {
            field: "muted_foreground",
            label_key: "theme_color_muted_foreground",
            value: palette.muted_foreground,
        },
        ColorSpec {
            field: "input",
            label_key: "theme_color_input",
            value: palette.input,
        },
        ColorSpec {
            field: "primary_hover",
            label_key: "theme_color_primary_hover",
            value: palette.primary_hover,
        },
        ColorSpec {
            field: "primary_active",
            label_key: "theme_color_primary_active",
            value: palette.primary_active,
        },
        ColorSpec {
            field: "danger",
            label_key: "theme_color_danger",
            value: palette.danger,
        },
        ColorSpec {
            field: "danger_foreground",
            label_key: "theme_color_danger_foreground",
            value: palette.danger_foreground,
        },
        ColorSpec {
            field: "popover",
            label_key: "theme_color_popover",
            value: palette.popover,
        },
        ColorSpec {
            field: "popover_foreground",
            label_key: "theme_color_popover_foreground",
            value: palette.popover_foreground,
        },
        ColorSpec {
            field: "selection",
            label_key: "theme_color_selection",
            value: palette.selection,
        },
        ColorSpec {
            field: "ring",
            label_key: "theme_color_ring",
            value: palette.ring,
        },
        ColorSpec {
            field: "list_hover",
            label_key: "theme_color_list_hover",
            value: palette.list_hover,
        },
        ColorSpec {
            field: "list_active",
            label_key: "theme_color_list_active",
            value: palette.list_active,
        },
        ColorSpec {
            field: "scrollbar_thumb",
            label_key: "theme_color_scrollbar_thumb",
            value: palette.scrollbar_thumb,
        },
    ]
}

struct ColorSpec {
    field: &'static str,
    label_key: &'static str,
    value: u32,
}

/// Round HSL conversion noise so editing another field does not darken unchanged colors.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Rounded color channels are clamped to the u8 range"
)]
fn hex_color(value: gpui_kit::Hsla) -> String {
    let value = value.to_rgb();
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(value.r),
        channel(value.g),
        channel(value.b)
    )
}

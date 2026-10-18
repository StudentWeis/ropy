//! File commands for user themes; I/O completes before updating the UI snapshot.

use gpui_kit::{AppContext, Context, Window};
use rust_i18n::t;

use super::RopyBoard;
use crate::gui::{settings::GlobalSettings, theme::ThemeCatalog};

impl RopyBoard {
    pub(crate) fn reload_themes(&mut self, cx: &mut Context<'_, Self>) {
        if self.theme_files.busy {
            return;
        }
        let Some(directory) = GlobalSettings::themes_directory(cx) else {
            self.theme_files.message = Some(t!("themes_location_failed").to_string());
            cx.notify();
            return;
        };
        self.theme_files.busy = true;
        self.theme_files.message = None;
        cx.notify();
        let catalog = ThemeCatalog::global(cx).clone();
        let task = cx.background_spawn(async move { catalog.reload(&directory) });
        cx.spawn(async move |board, cx| {
            let catalog = task.await;
            let _ = board.update(cx, |board, cx| {
                cx.set_global(catalog);
                board.theme_files.busy = false;
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn open_themes_folder(&mut self, window: &Window, cx: &mut Context<'_, Self>) {
        self.theme_file_command(false, window, cx);
    }

    pub(crate) fn export_theme(&mut self, window: &Window, cx: &mut Context<'_, Self>) {
        self.theme_file_command(true, window, cx);
    }

    fn theme_file_command(&mut self, export: bool, window: &Window, cx: &mut Context<'_, Self>) {
        if self.theme_files.busy {
            return;
        }
        let Some(directory) = GlobalSettings::themes_directory(cx) else {
            self.theme_files.message = Some(t!("themes_location_failed").to_string());
            cx.notify();
            return;
        };
        let catalog = ThemeCatalog::global(cx);
        let id = GlobalSettings::read(cx, |settings| {
            catalog.resolve(settings, window.appearance().into())
        });
        let id = self
            .theme_workspace
            .as_ref()
            .map_or(id, |workspace| workspace.selected.clone());
        let source = catalog.source(&id).unwrap_or_default().to_owned();
        self.theme_files.busy = true;
        self.theme_files.message = None;
        cx.notify();
        let task =
            cx.background_spawn(
                async move { std::fs::create_dir_all(&directory).map(|()| directory) },
            );
        cx.spawn_in(window, async move |board, cx| {
            let result = task.await;
            let result = match result {
                Ok(directory) => {
                    if export {
                        let prompt = cx.update(|_, cx| {
                            cx.prompt_for_new_path(&directory, Some("my-theme.toml"))
                        });
                        match prompt {
                            Ok(prompt) => match prompt.await {
                                Ok(Ok(Some(path))) => cx
                                    .background_spawn(async move {
                                        crate::gui::theme::storage::export(&path, &source)
                                    })
                                    .await
                                    .map_err(|error| error.to_string()),
                                Ok(Ok(None)) => Ok(()),
                                Ok(Err(error)) => Err(error.to_string()),
                                Err(error) => Err(error.to_string()),
                            },
                            Err(error) => Err(error.to_string()),
                        }
                    } else {
                        cx.update(|_, cx| cx.open_with_system(&directory))
                            .map_err(|error| error.to_string())
                    }
                }
                Err(error) => Err(error.to_string()),
            };
            let _ = board.update(cx, |board, cx| {
                board.theme_files.busy = false;
                if let Err(error) = result {
                    tracing::warn!(%error, "theme file command failed");
                    board.theme_files.message = Some(t!("themes_file_command_failed").to_string());
                }
                cx.notify();
            });
        })
        .detach();
    }
}

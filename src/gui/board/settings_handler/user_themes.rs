//! File commands for user themes; I/O completes before updating the UI snapshot.

use gpui_kit::{AppContext, Context, Window};
use rust_i18n::t;

use super::RopyBoard;
use crate::gui::{settings::GlobalSettings, theme::ThemeCatalog};

impl RopyBoard {
    pub(crate) fn reload_themes(&mut self, cx: &mut Context<'_, Self>) {
        if self.settings_editor.theme.busy {
            return;
        }
        let Some(directory) = GlobalSettings::themes_directory(cx) else {
            self.settings_editor.theme.message = Some(t!("themes_location_failed").to_string());
            cx.notify();
            return;
        };
        self.settings_editor.theme.busy = true;
        self.settings_editor.theme.message = None;
        cx.notify();
        let catalog = ThemeCatalog::global(cx).clone();
        let task = cx.background_spawn(async move { catalog.reload(&directory) });
        cx.spawn(async move |board, cx| {
            let catalog = task.await;
            let _ = board.update(cx, |board, cx| {
                cx.set_global(catalog);
                board.settings_editor.theme.busy = false;
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
        if self.settings_editor.theme.busy {
            return;
        }
        let Some(directory) = GlobalSettings::themes_directory(cx) else {
            self.settings_editor.theme.message = Some(t!("themes_location_failed").to_string());
            cx.notify();
            return;
        };
        let catalog = ThemeCatalog::global(cx);
        let id = GlobalSettings::read(cx, |settings| {
            catalog.resolve(settings, window.appearance().into())
        });
        let source = catalog.source(&id).unwrap_or_default().to_owned();
        self.settings_editor.theme.busy = true;
        self.settings_editor.theme.message = None;
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
                                        use std::io::Write;
                                        let directory = path
                                            .parent()
                                            .unwrap_or_else(|| std::path::Path::new("."));
                                        let mut file = tempfile::NamedTempFile::new_in(directory)?;
                                        file.write_all(source.as_bytes())?;
                                        file.as_file().sync_all()?;
                                        file.persist(path).map_err(|error| error.error)?;
                                        Ok::<(), std::io::Error>(())
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
                board.settings_editor.theme.busy = false;
                if let Err(error) = result {
                    tracing::warn!(%error, "theme file command failed");
                    board.settings_editor.theme.message =
                        Some(t!("themes_file_command_failed").to_string());
                }
                cx.notify();
            });
        })
        .detach();
    }
}

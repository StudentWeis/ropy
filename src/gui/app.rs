//! GUI-specific application resources: embedded assets, theme configuration,
//! and window creation.

use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
};

use gpui::{
    App, AppContext, AssetSource, Bounds, WindowBounds, WindowHandle, WindowKind, WindowOptions,
    rgb,
};
use gpui_component::{Root, ThemeMode as ComponentThemeMode, theme::Theme};
use rust_embed::RustEmbed;

use crate::{
    clipboard::LastCopyState,
    config::Settings,
    gui::{
        board::RopyBoard,
        constants::default_window_size,
        theme::{ThemeDefinition, ThemeId, ThemeMode},
    },
    repository::SharedRecords,
};

#[derive(RustEmbed)]
#[folder = "assets"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(|data| data.data))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}

/// Create the main application window.
pub fn create_window(
    cx: &mut App,
    shared_records: SharedRecords,
    last_copy: Arc<Mutex<LastCopyState>>,
    copy_tx: async_channel::Sender<crate::clipboard::CopyRequest>,
    is_silent: bool,
) -> WindowHandle<Root> {
    let bounds = Bounds::centered(None, default_window_size(), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::PopUp,
            titlebar: None,
            show: !is_silent, // When silent mode, do not show the window initially
            ..Default::default()
        },
        |window, cx| {
            // Apply the application theme based on settings
            let theme_id = Settings::read(cx, |s| s.theme.clone());
            set_app_theme(window, cx, &theme_id);

            let view = cx.new(|cx| RopyBoard::new(shared_records, last_copy, copy_tx, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        },
    )
    .unwrap_or_else(|e| {
        tracing::error!(error = %e, "fatal: failed to create window");
        std::process::exit(1);
    })
}

/// Set the application theme from a bundled theme definition.
pub fn set_app_theme(window: &mut gpui::Window, cx: &mut App, theme_id: &ThemeId) {
    let app_theme = ThemeDefinition::load_or_default(theme_id);
    let component_mode = match app_theme.mode() {
        ThemeMode::Light => ComponentThemeMode::Light,
        ThemeMode::Dark => ComponentThemeMode::Dark,
    };
    let palette = app_theme.palette();

    Theme::change(component_mode, Some(window), cx);

    let theme = Theme::global_mut(cx);
    theme.background = rgb(palette.background).into();
    theme.foreground = rgb(palette.foreground).into();
    theme.secondary = rgb(palette.secondary).into();
    theme.secondary_foreground = rgb(palette.secondary_foreground).into();
    theme.border = rgb(palette.border).into();
    theme.accent = rgb(palette.accent).into();
    theme.accent_foreground = rgb(palette.accent_foreground).into();
    theme.muted = rgb(palette.muted).into();
    theme.muted_foreground = rgb(palette.muted_foreground).into();
    theme.input = rgb(palette.input).into();
    theme.primary = rgb(palette.primary).into();
    theme.primary_foreground = rgb(palette.primary_foreground).into();
    theme.primary_hover = rgb(palette.primary_hover).into();
    theme.primary_active = rgb(palette.primary_active).into();
    theme.danger = rgb(palette.danger).into();
    theme.danger_foreground = rgb(palette.danger_foreground).into();
    theme.popover = rgb(palette.popover).into();
    theme.popover_foreground = rgb(palette.popover_foreground).into();
    theme.selection = rgb(palette.selection).into();
    theme.ring = rgb(palette.ring).into();
    theme.list_hover = rgb(palette.list_hover).into();
    theme.list_active = rgb(palette.list_active).into();
    theme.scrollbar_thumb = rgb(palette.scrollbar_thumb).into();
}

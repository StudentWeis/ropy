//! GUI-specific application resources: embedded assets, theme configuration,
//! and window creation.

use std::{
    borrow::Cow,
    sync::{Arc, Mutex, RwLock},
};

use gpui::{
    App, AppContext, AssetSource, Bounds, WindowBounds, WindowHandle, WindowKind, WindowOptions,
    px, rgb, size,
};
use gpui_component::{Root, ThemeMode, theme::Theme};
use rust_embed::RustEmbed;

use crate::{
    clipboard::LastCopyState,
    config::{AppTheme, Settings},
    gui::board::RopyBoard,
    repository::{ClipboardRecord, ClipboardRepository},
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
    shared_records: Arc<Mutex<Vec<ClipboardRecord>>>,
    repository: Option<Arc<ClipboardRepository>>,
    settings: Arc<RwLock<Settings>>,
    last_copy: Arc<Mutex<LastCopyState>>,
    copy_tx: async_channel::Sender<crate::clipboard::CopyRequest>,
    is_silent: bool,
) -> WindowHandle<Root> {
    let bounds = Bounds::centered(None, size(px(400.), px(600.0)), cx);
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
            let app_theme = &match settings.read() {
                Ok(g) => g,
                Err(e) => e.into_inner(),
            }
            .theme
            .get_theme();
            set_app_theme(window, cx, app_theme);

            let view = cx.new(|cx| {
                RopyBoard::new(
                    shared_records,
                    repository,
                    settings,
                    last_copy,
                    copy_tx,
                    window,
                    cx,
                )
            });
            cx.new(|cx| Root::new(view, window, cx))
        },
    )
    .unwrap_or_else(|e| {
        tracing::error!(error = %e, "fatal: failed to create window");
        std::process::exit(1);
    })
}

/// Set the application theme (light or dark)
pub fn set_app_theme(window: &mut gpui::Window, cx: &mut App, app_theme: &AppTheme) {
    match app_theme.get_theme() {
        AppTheme::Dark => {
            Theme::change(ThemeMode::Dark, Some(window), cx);
            let theme = Theme::global_mut(cx);
            theme.background = rgb(0x002d_2d2d).into();
            theme.foreground = rgb(0x00ff_ffff).into();
            theme.secondary = rgb(0x003d_3d3d).into();
            theme.secondary_foreground = rgb(0x00ff_ffff).into();
            theme.border = rgb(0x004d_4d4d).into();
            theme.accent = rgb(0x004d_4d4d).into();
            theme.muted_foreground = rgb(0x0088_8888).into();
            theme.input = rgb(0x0055_5555).into();
        }
        AppTheme::Light => {
            Theme::change(ThemeMode::Light, Some(window), cx);
            let theme = Theme::global_mut(cx);
            theme.background = rgb(0x00ff_ffff).into();
            theme.foreground = rgb(0x001a_1a1a).into();
            theme.secondary = rgb(0x00f5_f5f5).into();
            theme.secondary_foreground = rgb(0x001a_1a1a).into();
            theme.border = rgb(0x00e0_e0e0).into();
            theme.accent = rgb(0x00ad_d8e6).into();
            theme.muted_foreground = rgb(0x006b_6b6b).into();
            theme.input = rgb(0x00f0_f0f0).into();
        }
        AppTheme::System => todo!(),
    }
}

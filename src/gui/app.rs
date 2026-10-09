//! GUI-specific application resources: embedded assets, theme configuration,
//! and window creation.

use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
};

use gpui_kit::{
    App, AppContext, AssetSource, Bounds, WindowBackgroundAppearance, WindowBounds, WindowHandle,
    WindowKind, WindowOptions,
    component::{Root, ThemeMode as ComponentThemeMode, theme::Theme},
    rgb,
};
use rust_embed::RustEmbed;

use crate::{
    clipboard::CopyTracker,
    gui::{
        board::RopyBoard,
        constants::default_window_size,
        settings::GlobalSettings,
        theme::{ThemeDefinition, ThemeId, ThemeMode},
        utils::surface_with_opacity,
    },
    repository::SharedRecords,
};

pub(crate) const MAIN_WINDOW_TITLE: &str = "Ropy";

#[derive(RustEmbed)]
#[folder = "assets"]
pub(crate) struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(|data| data.data))
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<gpui_kit::SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}

/// Create the main application window.
///
/// The window is always created hidden — Ropy is a tray-resident clipboard
/// manager and is only revealed by the global hotkey or the tray menu,
/// regardless of how the process was launched.
pub(crate) fn create_window(
    cx: &mut App,
    shared_records: SharedRecords,
    last_copy: Arc<Mutex<CopyTracker>>,
    copy_tx: async_channel::Sender<crate::clipboard::CopyRequest>,
) -> WindowHandle<Root> {
    let bounds = Bounds::centered(None, default_window_size(), cx);
    let window_opacity_percent = GlobalSettings::read(cx, |s| s.window.opacity_percent);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::PopUp,
            titlebar: None,
            show: false,
            window_background: background_appearance_for_opacity(window_opacity_percent),
            ..Default::default()
        },
        |window, cx| {
            window.set_window_title(MAIN_WINDOW_TITLE);
            // Apply the application theme based on settings
            let theme_id = GlobalSettings::read(cx, |s| s.theme.clone());
            set_app_theme(window, cx, &theme_id, window_opacity_percent);
            apply_window_opacity(window, window_opacity_percent);

            let view = cx.new(|cx| RopyBoard::new(shared_records, last_copy, copy_tx, window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        },
    )
    .unwrap_or_else(|e| {
        tracing::error!(error = %e, "fatal: failed to create window");
        std::process::exit(1);
    })
}

const fn background_appearance_for_opacity(opacity_percent: u8) -> WindowBackgroundAppearance {
    if opacity_percent < 100 {
        WindowBackgroundAppearance::Transparent
    } else {
        WindowBackgroundAppearance::Opaque
    }
}

pub(crate) fn apply_window_opacity(window: &gpui_kit::Window, opacity_percent: u8) {
    window.set_background_appearance(background_appearance_for_opacity(opacity_percent));
}

/// Set the application theme from a bundled theme definition.
pub(crate) fn set_app_theme(
    window: &mut gpui_kit::Window,
    cx: &mut App,
    theme_id: &ThemeId,
    opacity_percent: u8,
) {
    let app_theme = ThemeDefinition::load_or_default(theme_id);
    let component_mode = match app_theme.mode() {
        ThemeMode::Light => ComponentThemeMode::Light,
        ThemeMode::Dark => ComponentThemeMode::Dark,
    };
    let palette = app_theme.palette();

    Theme::change(component_mode, Some(window), cx);

    let surface = |color| surface_with_opacity(color, opacity_percent);

    Theme::update(cx, |theme| {
        theme.background = surface(rgb(palette.background).into());
        theme.foreground = rgb(palette.foreground).into();
        theme.secondary = surface(rgb(palette.secondary).into());
        theme.secondary_foreground = rgb(palette.secondary_foreground).into();
        theme.border = surface(rgb(palette.border).into());
        theme.accent = surface(rgb(palette.accent).into());
        theme.accent_foreground = rgb(palette.accent_foreground).into();
        theme.muted = surface(rgb(palette.muted).into());
        theme.muted_foreground = rgb(palette.muted_foreground).into();
        theme.input = surface(rgb(palette.input).into());
        theme.primary = surface(rgb(palette.primary).into());
        theme.primary_foreground = rgb(palette.primary_foreground).into();
        theme.primary_hover = surface(rgb(palette.primary_hover).into());
        theme.primary_active = surface(rgb(palette.primary_active).into());
        theme.danger = surface(rgb(palette.danger).into());
        theme.danger_foreground = rgb(palette.danger_foreground).into();
        theme.popover = surface(rgb(palette.popover).into());
        theme.popover_foreground = rgb(palette.popover_foreground).into();
        theme.selection = surface(rgb(palette.selection).into());
        theme.ring = surface(rgb(palette.ring).into());
        theme.list_hover = surface(rgb(palette.list_hover).into());
        theme.list_active = surface(rgb(palette.list_active).into());
        theme.scrollbar_thumb = surface(rgb(palette.scrollbar_thumb).into());
        theme.notification.placement = gpui_kit::Anchor::BottomRight;
        theme.notification.margins.right = gpui_kit::px(12.);
    });
}

#[cfg(test)]
mod tests {
    use gpui_kit::{Anchor, TestAppContext, rgb, solid_background};
    use rstest::rstest;

    use super::set_app_theme;
    use crate::gui::{theme::ThemeDefinition, utils::surface_with_opacity};

    #[rstest]
    #[case(0)]
    #[case(50)]
    #[case(100)]
    fn test_app_theme_switch_updates_component_tokens_and_base(#[case] opacity: u8) {
        let mut cx = TestAppContext::single();
        cx.update(gpui_kit::init);
        let cx = cx.add_empty_window();
        for theme_id in crate::gui::theme::available_themes() {
            cx.update(|window, cx| {
                set_app_theme(window, cx, &theme_id, opacity);
                let definition = ThemeDefinition::load_or_default(&theme_id);
                let palette = definition.palette();
                let expected = surface_with_opacity(rgb(palette.background).into(), opacity);
                let component = gpui_kit::component::Theme::global(cx);
                assert_eq!(component.background, expected);
                assert_eq!(
                    component.tokens.background.background,
                    solid_background(expected)
                );
                assert_eq!(
                    gpui_kit::base::Theme::global(cx).tokens.colors.background,
                    expected
                );
                assert_eq!(component.notification.placement, Anchor::BottomRight);
            });
        }
    }
}

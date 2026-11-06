//! GUI-specific application resources: embedded assets, theme configuration,
//! and window creation.

use std::{
    borrow::Cow,
    sync::{Arc, Mutex},
};

use gpui_kit::{
    App, AppContext, AssetSource, Bounds, Entity, Window, WindowBackgroundAppearance, WindowBounds,
    WindowHandle, WindowKind, WindowOptions,
    component::{Colorize, Root, ThemeMode as ComponentThemeMode, theme::Theme},
    rgb,
};
use rust_embed::RustEmbed;

use crate::{
    clipboard::CopyTracker,
    gui::{
        SharedRecords,
        board::RopyBoard,
        constants::default_window_size,
        settings::GlobalSettings,
        surface_with_opacity,
        theme::{ThemeDefinition, ThemeId, ThemeMode},
    },
};

pub(crate) const MAIN_WINDOW_TITLE: &str = "Ropy";

#[cfg(target_os = "linux")]
impl gpui_kit::Global for crate::linux::hyprland::PopupRules {}

#[derive(RustEmbed)]
#[folder = "assets"]
#[exclude = "locales/*"]
pub(crate) struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> gpui_kit::Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path)
            .or_else(|| gpui_kit::assets::Assets::get(path))
            .map(|data| data.data))
    }

    fn list(&self, path: &str) -> gpui_kit::Result<Vec<gpui_kit::SharedString>> {
        let mut paths: Vec<_> = Self::iter()
            .chain(gpui_kit::assets::Assets::iter())
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect();
        paths.sort();
        paths.dedup();
        Ok(paths)
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
    open_window(cx, |window, cx| {
        cx.new(|cx| RopyBoard::new(shared_records, last_copy, copy_tx, window, cx))
    })
    .unwrap_or_else(|e| {
        tracing::error!(error = %e, "fatal: failed to create window");
        std::process::exit(1);
    })
}

pub(crate) fn reopen_window(
    cx: &mut App,
    board: Entity<RopyBoard>,
) -> gpui_kit::Result<WindowHandle<Root>> {
    open_window(cx, |window, cx| {
        board.update(cx, |board, cx| board.attach_window(window, cx));
        board
    })
}

fn open_window(
    cx: &mut App,
    build_board: impl FnOnce(&mut Window, &mut App) -> Entity<RopyBoard>,
) -> gpui_kit::Result<WindowHandle<Root>> {
    let bounds = Bounds::centered(None, default_window_size(), cx);
    #[cfg(target_os = "linux")]
    if let Some(rules) = cx.try_global::<crate::linux::hyprland::PopupRules>()
        && let Err(error) = rules.prepare(bounds.size.width.into(), bounds.size.height.into())
    {
        tracing::warn!(%error, "failed to prepare Hyprland floating popup");
    }
    let window_opacity_percent = GlobalSettings::read(cx, |s| s.window.opacity_percent);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::PopUp,
            app_id: Some("com.ropy.app".into()),
            titlebar: None,
            show: false,
            window_background: background_appearance_for_opacity(window_opacity_percent),
            ..Default::default()
        },
        |window, cx| {
            window.set_window_title(MAIN_WINDOW_TITLE);
            apply_current_theme(window, cx, window_opacity_percent);
            apply_window_opacity(window, window_opacity_percent);

            let view = build_board(window, cx);
            cx.new(|cx| Root::new(view, window, cx))
        },
    )
}

const fn background_appearance_for_opacity(opacity_percent: u8) -> WindowBackgroundAppearance {
    if opacity_percent < 100 {
        WindowBackgroundAppearance::Transparent
    } else {
        WindowBackgroundAppearance::Opaque
    }
}

pub(crate) fn apply_window_opacity(window: &Window, opacity_percent: u8) {
    window.set_background_appearance(background_appearance_for_opacity(opacity_percent));
}

/// Apply the saved appearance strategy using this window's OS appearance.
pub(crate) fn apply_current_theme(window: &mut Window, cx: &mut App, opacity_percent: u8) {
    let mode = window.appearance().into();
    let id = GlobalSettings::read(cx, |settings| {
        super::theme::ThemeCatalog::global(cx).resolve(settings, mode)
    });
    set_app_theme(window, cx, &id, opacity_percent);
}

/// Set the application theme from a catalog or bundled theme definition.
/// Surface tokens include window opacity; consumers must use them unchanged.
pub(crate) fn set_app_theme(
    window: &mut Window,
    cx: &mut App,
    theme_id: &ThemeId,
    opacity_percent: u8,
) {
    let app_theme = super::theme::ThemeCatalog::global(cx)
        .get(theme_id)
        .cloned()
        .unwrap_or_else(|| ThemeDefinition::load_or_default(theme_id));
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

        // Theme::update reconciles same-name colors/tokens, but does not rerun
        // schema fallbacks. Project our compact palette onto the tokens read by
        // the components Ropy uses, rather than leaving the framework palette.
        theme.secondary_hover = theme.list_hover;
        theme.secondary_active = theme.list_active;
        theme.button = theme.secondary;
        theme.button_foreground = theme.secondary_foreground;
        theme.button_hover = theme.list_hover;
        theme.button_active = theme.list_active;
        theme.button_primary = theme.primary;
        theme.button_primary_foreground = theme.primary_foreground;
        theme.button_primary_hover = theme.primary_hover;
        theme.button_primary_active = theme.primary_active;
        theme.button_secondary = theme.secondary;
        theme.button_secondary_foreground = theme.secondary_foreground;
        theme.button_secondary_hover = theme.secondary_hover;
        theme.button_secondary_active = theme.secondary_active;
        theme.danger_hover = theme.danger.darken(0.05);
        theme.danger_active = theme.danger.darken(0.1);
        theme.button_danger = theme.danger;
        theme.button_danger_foreground = theme.danger_foreground;
        theme.button_danger_hover = theme.danger_hover;
        theme.button_danger_active = theme.danger_active;
        theme.switch = theme.secondary_active;
        theme.switch_thumb = theme.background;
        theme.slider_bar = theme.primary;
        theme.slider_thumb = surface(rgb(palette.primary_foreground).into());
        theme.colors.list = theme.background;
        theme.scrollbar = theme.background;
        theme.scrollbar_thumb_hover = theme.scrollbar_thumb;
        theme.drag_border = theme.ring;
        // A caret is foreground, not a translucent window surface.
        theme.caret = rgb(palette.primary).into();
        theme.notification.placement = gpui_kit::Anchor::BottomRight;
        theme.notification.margins.right = gpui_kit::px(12.);
    });
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use gpui_kit::{Anchor, AssetSource, TestAppContext, rgb, solid_background};
    use rstest::rstest;

    use super::{Assets, set_app_theme};
    use crate::gui::{theme::ThemeDefinition, utils::surface_with_opacity};

    #[gpui_kit::test]
    #[expect(
        clippy::unwrap_used,
        reason = "test window setup and update must succeed"
    )]
    fn test_board_surfaces_apply_window_opacity_once(cx: &mut TestAppContext) {
        use std::sync::{Arc, Mutex, RwLock};

        use gpui_kit::{AppContext, component::ActiveTheme, test::TestWindowExt};

        use crate::{
            clipboard::CopyTracker,
            config::Settings,
            gui::{board::RopyBoard, repository::GlobalRepository, settings::GlobalSettings},
            repository::{ClipboardRecord, ContentType},
        };

        cx.update(|cx| {
            gpui_kit::init(cx);
            let mut settings = Settings::default();
            settings.window.opacity_percent = 50;
            cx.set_global(GlobalSettings::new(settings));
            cx.set_global(GlobalRepository::new(None));
        });
        let (handle, _) = cx.update(|cx| {
            gpui_kit::open_window(gpui_kit::WindowOptions::default(), cx, |window, cx| {
                set_app_theme(window, cx, &crate::gui::theme::ThemeId::default(), 50);
                let (tx, _) = async_channel::bounded(1);
                cx.new(|cx| {
                    let mut board = RopyBoard::new(
                        Arc::new(RwLock::new(vec![ClipboardRecord::new(
                            1,
                            "opacity regression".into(),
                            chrono::Local::now(),
                            ContentType::Text,
                        )])),
                        Arc::new(Mutex::new(CopyTracker::default())),
                        tx,
                        window,
                        cx,
                    );
                    board.activated = true;
                    board.pinned = true;
                    board.filter_state.search_options.case_sensitive = true;
                    board
                })
            })
            .unwrap()
        });
        cx.update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            window.hover("search-case-sensitive-btn", cx);
            let quads = window.painted_quads();
            for (expected, minimum_surfaces) in [
                (cx.theme().background, 1),
                (cx.theme().secondary, 4),
                (cx.theme().accent, 1),
            ] {
                let colors: Vec<_> = quads
                    .iter()
                    .filter_map(|quad| quad.background.as_solid())
                    .filter(|color| color.alpha(1.) == expected.alpha(1.))
                    .collect();
                assert!(
                    colors.len() >= minimum_surfaces,
                    "expected at least {minimum_surfaces} surfaces for {expected:?}, found {}",
                    colors.len()
                );
                for color in colors {
                    assert!(
                        (color.a - 0.5).abs() < 0.001,
                        "50% surface became {}",
                        color.a
                    );
                }
            }
        })
        .unwrap();
    }

    #[rstest]
    #[case("icons/chevron-down.svg")]
    #[case("icons/sun.svg")]
    #[case("icons/moon.svg")]
    fn test_settings_icons_load_from_application_assets(#[case] path: &str) {
        assert_matches!(Assets.load(path), Ok(Some(_)), "missing {path}");
        assert!(
            Assets
                .list("icons/")
                .is_ok_and(|paths| paths.iter().any(|p| p == path))
        );
    }

    #[test]
    fn test_bundled_themes_actual_text_pairs_have_readable_contrast() {
        fn luminance(color: gpui_kit::Hsla) -> f64 {
            let color = gpui_kit::Rgba::from(color);
            [color.r, color.g, color.b]
                .into_iter()
                .zip([0.2126, 0.7152, 0.0722])
                .map(|(channel, weight)| {
                    let channel = f64::from(channel);
                    let linear = if channel <= 0.04045 {
                        channel / 12.92
                    } else {
                        ((channel + 0.055) / 1.055).powf(2.4)
                    };
                    linear * weight
                })
                .sum()
        }
        let mut failures = Vec::new();
        let mut cx = TestAppContext::single();
        cx.update(gpui_kit::init);
        let cx = cx.add_empty_window();
        // Check opaque bundled themes through their production projection, not
        // user themes or translucent surfaces with an unknown desktop backdrop.
        for id in crate::gui::theme::available_themes() {
            cx.update(|window, cx| {
                set_app_theme(window, cx, &id, 100);
                let p = gpui_kit::component::Theme::global(cx);
                let mut pairs = vec![
                    (p.foreground, p.background),
                    (p.secondary_foreground, p.secondary),
                    (p.accent_foreground, p.accent),
                    (p.popover_foreground, p.popover),
                    (p.foreground, p.selection),
                    (p.button_primary_foreground, p.button_primary),
                    (p.button_primary_foreground, p.button_primary_hover),
                    (p.button_primary_foreground, p.button_primary_active),
                    (p.button_danger_foreground, p.button_danger),
                    (p.button_danger_foreground, p.button_danger_hover),
                    (p.button_danger_foreground, p.button_danger_active),
                ];
                // Rows, popup metadata and selection surfaces keep secondary
                // text readable, including the selected row's accent surface.
                pairs.extend(
                    [
                        p.background,
                        p.secondary,
                        p.accent,
                        p.muted,
                        p.popover,
                        p.list_hover,
                        p.list_active,
                        p.selection,
                    ]
                    .map(|surface| (p.muted_foreground, surface)),
                );
                for (text, surface) in pairs {
                    let (a, b) = (luminance(text), luminance(surface));
                    let ratio = (a.max(b) + 0.05) / (a.min(b) + 0.05);
                    if ratio < 4.5 {
                        failures.push(format!(
                            "{}: {text:?} on {surface:?} = {ratio:.2}",
                            id.code()
                        ));
                    }
                }
            });
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }

    #[rstest]
    #[case(0)]
    #[case(50)]
    #[case(100)]
    fn test_app_theme_switch_updates_component_tokens_and_base(
        #[case] opacity: u8,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut cx = TestAppContext::single();
        cx.update(gpui_kit::init);
        // Use a user palette with every color distinct from the framework
        // defaults, so mode changes cannot accidentally satisfy the assertions.
        let custom_id = crate::gui::theme::ThemeId::new("user:projection-test");
        let mut source: toml::Value =
            toml::from_str(include_str!("../../assets/themes/ropy-light.toml"))?;
        for (index, (key, value)) in source
            .as_table_mut()
            .ok_or("theme table")?
            .iter_mut()
            .enumerate()
        {
            if key != "mode" && key != "theme_name" {
                *value = toml::Value::String(format!("#{:06x}", 0x0012_3456 + index * 0x0004_0302));
            }
        }
        let source = toml::to_string(&source)?;
        let custom = ThemeDefinition::parse(&custom_id, &source)?;
        cx.update(|cx| {
            let mut catalog = crate::gui::theme::ThemeCatalog::default();
            catalog.insert(custom, source);
            cx.set_global(catalog);
        });
        let cx = cx.add_empty_window();
        let theme_ids = crate::gui::theme::available_themes();
        // Apply the custom palette on both sides of bundled mode switches.
        for theme_id in std::iter::once(custom_id.clone())
            .chain(theme_ids)
            .chain(std::iter::once(custom_id))
        {
            cx.update(|window, cx| -> Result<(), Box<dyn std::error::Error>> {
                set_app_theme(window, cx, &theme_id, opacity);
                let definition = crate::gui::theme::ThemeCatalog::global(cx)
                    .get(&theme_id)
                    .ok_or("registered theme")?;
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
                // These are the production tokens read by Button (including
                // hover/press), Switch, Slider, popup lists and Base scrollbars.
                for (token, color) in [
                    (component.tokens.button, palette.secondary),
                    (component.tokens.button_hover, palette.list_hover),
                    (component.tokens.button_active, palette.list_active),
                    (component.tokens.button_primary, palette.primary),
                    (component.tokens.button_primary_hover, palette.primary_hover),
                    (
                        component.tokens.button_primary_active,
                        palette.primary_active,
                    ),
                    (component.tokens.button_secondary, palette.secondary),
                    (component.tokens.button_secondary_hover, palette.list_hover),
                    (
                        component.tokens.button_secondary_active,
                        palette.list_active,
                    ),
                    (component.tokens.button_danger, palette.danger),
                    (component.tokens.switch, palette.list_active),
                    (component.tokens.switch_thumb, palette.background),
                    (component.tokens.slider_bar, palette.primary),
                    (component.tokens.slider_thumb, palette.primary_foreground),
                    (component.tokens.list, palette.background),
                    (component.tokens.scrollbar, palette.background),
                    (
                        component.tokens.scrollbar_thumb_hover,
                        palette.scrollbar_thumb,
                    ),
                ] {
                    assert_eq!(
                        token.background,
                        solid_background(surface_with_opacity(rgb(color).into(), opacity)),
                        "{theme_id:?} at {opacity}%"
                    );
                }
                for (actual, color) in [
                    (component.secondary_foreground, palette.secondary_foreground),
                    (component.accent_foreground, palette.accent_foreground),
                    (component.muted_foreground, palette.muted_foreground),
                    (component.danger_foreground, palette.danger_foreground),
                    (component.popover_foreground, palette.popover_foreground),
                    (component.button_foreground, palette.secondary_foreground),
                    (
                        component.button_primary_foreground,
                        palette.primary_foreground,
                    ),
                    (
                        component.button_secondary_foreground,
                        palette.secondary_foreground,
                    ),
                    (
                        component.button_danger_foreground,
                        palette.danger_foreground,
                    ),
                ] {
                    assert_eq!(actual, rgb(color).into(), "text must remain opaque");
                }
                let base = gpui_kit::base::Theme::global(cx);
                assert_eq!(base.tokens.colors.primary, component.primary);
                assert_eq!(
                    base.tokens.colors.muted_foreground,
                    component.muted_foreground
                );
                assert_eq!(base.resizable.handle, Some(component.border));
                assert_eq!(component.notification.placement, Anchor::BottomRight);
                Ok(())
            })?;
        }
        Ok(())
    }
}

//! Isolated, deterministic renders of the production board for documentation.

use std::{
    cell::RefCell,
    ffi::OsString,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, Mutex, RwLock},
};

use chrono::{Local, TimeZone};
use gpui_kit::{
    App, AppContext, Bounds, Context, Entity, ImgResourceLoader, IntoElement, Render, Resource,
    Window, WindowBounds, WindowKind, WindowOptions, div,
    prelude::{InteractiveElement, ParentElement, Styled},
};
use image::{Rgba, RgbaImage};
use thiserror::Error;

use crate::{
    clipboard::CopyTracker,
    config::{LayoutMode, Settings},
    gui::{
        Assets, app::set_app_theme, board::RopyBoard, repository::GlobalRepository,
        settings::GlobalSettings, theme::ThemeId,
    },
    i18n::{I18n, Language},
    repository::{ClipboardRecord, ContentType},
};

const GAP: u32 = 12;

#[derive(Debug)]
pub(crate) enum PromoCommand {
    Capture {
        theme: ThemeId,
        language: Language,
        ready: PathBuf,
    },
    Compose {
        output: PathBuf,
        inputs: [PathBuf; 4],
    },
}

#[derive(Debug, Error)]
pub(crate) enum PromoError {
    #[error("usage: --promo THEME LOCALE READY_FILE | --compose-promo OUTPUT PNG PNG PNG PNG")]
    Arguments,
    #[error("unknown promotional theme or locale")]
    UnknownPreset,
    #[error("promotional capture I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("promotional image failed: {0}")]
    Image(#[from] image::ImageError),
    #[error("four nonblank, equally sized screenshots are required")]
    InvalidScreenshots,
    #[error("failed to create promotional window: {0}")]
    Window(String),
    #[error("failed to load promotional logo: {0}")]
    Logo(String),
    #[error("could not resolve the fixed demonstration date in the local timezone")]
    Timestamp,
}

impl PromoCommand {
    pub(crate) fn parse(args: &[OsString]) -> Result<Option<Self>, PromoError> {
        match args.first().and_then(|arg| arg.to_str()) {
            Some("--promo") => {
                let [_, theme, language, ready] = args else {
                    return Err(PromoError::Arguments);
                };
                let theme = ThemeId::new(theme.to_str().ok_or(PromoError::Arguments)?);
                let language = Language::new(language.to_str().ok_or(PromoError::Arguments)?);
                if !crate::gui::theme::available_themes().contains(&theme)
                    || !crate::i18n::language::available_languages().contains(&language)
                {
                    return Err(PromoError::UnknownPreset);
                }
                Ok(Some(Self::Capture {
                    theme,
                    language,
                    ready: ready.into(),
                }))
            }
            Some("--compose-promo") => {
                let [_, output, a, b, c, d] = args else {
                    return Err(PromoError::Arguments);
                };
                Ok(Some(Self::Compose {
                    output: output.into(),
                    inputs: [a.into(), b.into(), c.into(), d.into()],
                }))
            }
            _ => Ok(None),
        }
    }

    pub(crate) fn run(self) -> Result<(), PromoError> {
        match self {
            Self::Capture {
                theme,
                language,
                ready,
            } => capture(theme, language, ready),
            Self::Compose { output, inputs } => compose(&output, &inputs),
        }
    }
}

fn demo_settings(theme: ThemeId, language: Language) -> Settings {
    let mut settings = Settings::default();
    settings.theme = theme;
    settings.language = language;
    settings.layout.mode = LayoutMode::List;
    settings.window.opacity_percent = 100;
    settings.autostart.enabled = false;
    settings.update.auto_check = false;
    settings.preview.hover_preview_enabled = false;
    settings.preview.space_preview_enabled = false;
    settings
}

fn demo_records(logo: &Path, i18n: &I18n) -> Result<Vec<ClipboardRecord>, PromoError> {
    // A local wall-clock value keeps the displayed date identical in every timezone.
    let created_at = Local
        .with_ymd_and_hms(2026, 4, 2, 13, 23, 14)
        .earliest()
        .ok_or(PromoError::Timestamp)?;
    Ok([
        (i18n.t("promo_name"), ContentType::Text),
        (i18n.t("promo_description"), ContentType::Text),
        (i18n.t("promo_code"), ContentType::Text),
        (logo.to_string_lossy().into_owned(), ContentType::Image),
    ]
    .into_iter()
    .zip(0u32..)
    .map(|((content, content_type), ix)| {
        ClipboardRecord::new(
            u64::from(ix) + 1,
            content,
            created_at - chrono::Duration::seconds(i64::from(ix)),
            content_type,
        )
    })
    .collect())
}

struct PromoView {
    board: Entity<RopyBoard>,
}

impl Render for PromoView {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        // The real board stays behind an invisible input shield: no button, hover,
        // scroll or keyboard path can persist settings or access OS services.
        div()
            .size_full()
            .relative()
            .capture_key_down(|_, _, cx| cx.stop_propagation())
            .capture_key_up(|_, _, cx| cx.stop_propagation())
            .child(self.board.clone())
            .child(
                div()
                    .id("promo-input-shield")
                    .absolute()
                    .inset_0()
                    .occlude()
                    .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                    .capture_any_mouse_up(|_, _, cx| cx.stop_propagation())
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation()),
            )
    }
}

type CaptureFailure = Rc<RefCell<Option<PromoError>>>;

fn signal_when_rendered(
    window: &mut Window,
    logo: Resource,
    ready: PathBuf,
    failure: CaptureFailure,
) {
    window.refresh();
    window.on_next_frame(move |window, cx| {
        match window.get_asset::<ImgResourceLoader>(&logo, cx) {
            Some(Ok(_)) => {
                window.refresh();
                window.on_next_frame(move |_, cx| {
                    if let Err(error) = std::fs::write(ready, std::process::id().to_string()) {
                        *failure.borrow_mut() = Some(error.into());
                        cx.quit();
                    }
                });
            }
            Some(Err(error)) => {
                *failure.borrow_mut() = Some(PromoError::Logo(error.to_string()));
                cx.quit();
            }
            None => signal_when_rendered(window, logo, ready, failure),
        }
    });
}

fn open_promo_window(
    records: Vec<ClipboardRecord>,
    cx: &mut App,
) -> gpui_kit::Result<(gpui_kit::AnyWindowHandle, Entity<PromoView>)> {
    let bounds = Bounds::centered(None, crate::gui::default_window_size(), cx);
    let result = gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::PopUp,
            titlebar: None,
            show: true,
            focus: false,
            is_resizable: false,
            ..WindowOptions::default()
        },
        cx,
        |window, cx| {
            window.set_window_title(crate::gui::app::MAIN_WINDOW_TITLE);
            let theme = GlobalSettings::read(cx, |s| s.theme.clone());
            set_app_theme(window, cx, &theme, 100);
            let (tx, rx) = async_channel::bounded(1);
            drop(rx);
            let board = cx.new(|cx| {
                let mut board = RopyBoard::new(
                    Arc::new(RwLock::new(records)),
                    Arc::new(Mutex::new(CopyTracker::default())),
                    tx,
                    window,
                    cx,
                );
                board.activated = true;
                board.pinned = true;
                board
            });
            cx.new(|_| PromoView { board })
        },
    )?;
    Ok(result)
}

fn capture(theme: ThemeId, language: Language, ready: PathBuf) -> Result<(), PromoError> {
    let fixture = tempfile::tempdir_in(
        ready
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new(".")),
    )?;
    let logo = fixture.path().join("logo.png");
    let asset = Assets::get("logo.png")
        .ok_or_else(|| PromoError::Logo("bundled logo is missing".into()))?;
    std::fs::write(&logo, asset.data)?;
    let settings = demo_settings(theme, language);
    let i18n = I18n::load_i18n(settings.language.clone());
    let records = demo_records(&logo, &i18n)?;
    let failure = CaptureFailure::default();
    let failure_for_app = failure.clone();
    gpui_kit::application().with_assets(Assets).run(move |cx| {
        #[cfg(target_os = "macos")]
        crate::gui::set_activation_policy_accessory();
        gpui_kit::init(cx);
        cx.set_global(GlobalSettings::new(settings));
        cx.set_global(i18n);
        cx.set_global(GlobalRepository::new(None));
        match open_promo_window(records, cx) {
            Ok((handle, _)) => {
                let _ = handle.update(cx, |_, window, cx| {
                    window.activate_window();
                    cx.activate(true);
                    window.refresh();
                    signal_when_rendered(
                        window,
                        Resource::Path(logo.into()),
                        ready,
                        failure_for_app,
                    );
                });
            }
            Err(error) => {
                *failure_for_app.borrow_mut() = Some(PromoError::Window(error.to_string()));
                cx.quit();
            }
        }
    });
    failure.take().map_or(Ok(()), Err)
}

fn compose(output: &Path, inputs: &[PathBuf; 4]) -> Result<(), PromoError> {
    let images = inputs
        .iter()
        .map(|path| image::open(path).map(image::DynamicImage::into_rgba8))
        .collect::<Result<Vec<_>, _>>()?;
    let (width, height) = images[0].dimensions();
    if width == 0
        || height == 0
        || images.iter().any(|image| {
            image.dimensions() != (width, height)
                || image.pixels().all(|pixel| pixel == image.get_pixel(0, 0))
        })
    {
        return Err(PromoError::InvalidScreenshots);
    }
    let out_width = width
        .checked_mul(2)
        .and_then(|v| v.checked_add(GAP * 3))
        .ok_or(PromoError::InvalidScreenshots)?;
    let out_height = height
        .checked_mul(2)
        .and_then(|v| v.checked_add(GAP * 3))
        .ok_or(PromoError::InvalidScreenshots)?;
    let mut composite = RgbaImage::from_pixel(out_width, out_height, Rgba([232, 234, 237, 255]));
    for (image, (x, y)) in images.iter().zip([
        (GAP, GAP),
        (width + GAP * 2, GAP),
        (GAP, height + GAP * 2),
        (width + GAP * 2, height + GAP * 2),
    ]) {
        image::imageops::overlay(&mut composite, image, i64::from(x), i64::from(y));
    }
    composite.save(output)?;
    Ok(())
}

#[cfg(test)]
mod tests;

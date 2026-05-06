#[cfg(target_os = "linux")]
use gpui::AppContext;
use gpui::{App, BorrowAppContext, Global, ReadGlobal, WindowHandle};
use gpui_component::Root;
#[cfg(not(target_os = "linux"))]
use tray_icon::TrayIconEvent;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuId, MenuItem},
};

use crate::{constants::APP_NAME, i18n::I18n};

/// Fixed menu IDs so event handlers remain valid after menu rebuilds.
const TRAY_SHOW_ID: &str = "tray_show";
const TRAY_QUIT_ID: &str = "tray_quit";
const TRAY_ICON_SIZE: u32 = 32;

#[derive(Default)]
pub(crate) struct TrayState {
    tray_icon: Option<TrayIcon>,
}

impl Global for TrayState {}

impl TrayState {
    pub(crate) fn register(cx: &mut App) {
        cx.set_global(Self::default());
    }

    fn replace(&mut self, tray_icon: Option<TrayIcon>) {
        self.tray_icon = tray_icon;
    }

    fn rebuild_menu(&self, i18n: &I18n) -> Result<bool, Box<dyn std::error::Error>> {
        let Some(tray) = self.tray_icon.as_ref() else {
            return Ok(false);
        };

        let menu = build_tray_menu(i18n)?;
        tray.set_menu(Some(Box::new(menu)));
        Ok(true)
    }

    /// Install the platform tray handle into the global state.
    ///
    /// On Linux, tray initialization happens on the GTK thread and the icon is
    /// intentionally leaked there, so this remains `None` and menu refreshes are
    /// a no-op.
    pub(crate) fn install(cx: &mut App, tray_icon: Option<TrayIcon>) {
        cx.update_global::<Self, _>(|state: &mut Self, _cx| {
            state.replace(tray_icon);
        });
    }

    pub(crate) fn refresh_menu(cx: &App) {
        let i18n = I18n::global(cx);
        if let Err(e) = Self::global(cx).rebuild_menu(i18n) {
            tracing::warn!(error = %e, "failed to rebuild tray menu");
        }
    }
}

pub(crate) fn build_tray_menu(i18n: &I18n) -> Result<Menu, Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(TRAY_SHOW_ID, i18n.t("tray_show"), true, None);
    let quit_item = MenuItem::with_id(TRAY_QUIT_ID, i18n.t("tray_quit"), true, None);

    let tray_menu = Menu::new();
    tray_menu.append(&show_item)?;
    tray_menu.append(&quit_item)?;
    Ok(tray_menu)
}

pub(crate) fn init_tray(
    i18n: &I18n,
) -> Result<(TrayIcon, MenuId, MenuId), Box<dyn std::error::Error>> {
    let tray_menu = build_tray_menu(i18n)?;
    let show_id = MenuId::new(TRAY_SHOW_ID);
    let quit_id = MenuId::new(TRAY_QUIT_ID);

    let icon = create_icon()?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip(APP_NAME)
        .with_icon(icon)
        .with_menu_on_left_click(false)
        .build()?;

    Ok((tray, show_id, quit_id))
}

fn create_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let asset = super::app::Assets::get("logo.png").ok_or("Failed to find embedded logo.png")?;
    let (rgba, width, height) = load_tray_icon_rgba(&asset.data)?;
    Icon::from_rgba(rgba, width, height).map_err(|e| format!("Failed to create icon: {e:?}").into())
}

fn load_tray_icon_rgba(bytes: &[u8]) -> Result<(Vec<u8>, u32, u32), image::ImageError> {
    let img = image::load_from_memory(bytes)?;
    let resized = img
        .resize(
            TRAY_ICON_SIZE,
            TRAY_ICON_SIZE,
            image::imageops::FilterType::Lanczos3,
        )
        .to_rgba8();

    if resized.width() == TRAY_ICON_SIZE && resized.height() == TRAY_ICON_SIZE {
        return Ok((resized.into_raw(), TRAY_ICON_SIZE, TRAY_ICON_SIZE));
    }

    let width = resized.width();
    let height = resized.height();
    let mut canvas = image::RgbaImage::new(TRAY_ICON_SIZE, TRAY_ICON_SIZE);
    let offset_x = i64::from((TRAY_ICON_SIZE - width) / 2);
    let offset_y = i64::from((TRAY_ICON_SIZE - height) / 2);
    image::imageops::overlay(&mut canvas, &resized, offset_x, offset_y);

    Ok((canvas.into_raw(), TRAY_ICON_SIZE, TRAY_ICON_SIZE))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TrayEvent {
    Show,
    Quit,
}

pub(crate) fn start_tray_handler(
    i18n: &I18n,
    cx: &App,
    window_handle: WindowHandle<Root>,
) -> Option<TrayIcon> {
    let (tx, rx) = async_channel::unbounded();
    let tray = spawn_tray_platform(i18n, cx, tx);
    spawn_tray_event_loop(cx, rx, window_handle);
    tray
}

#[cfg(not(target_os = "linux"))]
fn spawn_tray_platform(
    i18n: &I18n,
    _cx: &App,
    tx: async_channel::Sender<TrayEvent>,
) -> Option<TrayIcon> {
    start_tray_handler_inner(i18n, tx)
}

#[cfg(target_os = "linux")]
fn spawn_tray_platform(
    i18n: &I18n,
    cx: &App,
    tx: async_channel::Sender<TrayEvent>,
) -> Option<TrayIcon> {
    // On Linux the tray must be initialized on the GTK thread. We cannot
    // return the TrayIcon out of the spawned task, so we leak it there and
    // report `None` back to the caller.
    let i18n = i18n.clone();
    cx.background_spawn(async move {
        if let Err(e) = gtk::init() {
            tracing::error!(error = %e, "failed to init gtk modules; tray disabled");
            return;
        }
        if let Some(tray) = start_tray_handler_inner(&i18n, tx) {
            Box::leak(Box::new(tray));
        }
        gtk::main();
    })
    .detach();
    None
}

fn spawn_tray_event_loop(
    cx: &App,
    rx: async_channel::Receiver<TrayEvent>,
    window_handle: WindowHandle<Root>,
) {
    cx.spawn(async move |async_app| {
        while let Ok(event) = rx.recv().await {
            match event {
                TrayEvent::Show => {
                    let _ = async_app.update(|cx| send_active_action(window_handle, cx));
                }
                TrayEvent::Quit => {
                    let _ = async_app.update(|cx| cx.quit());
                }
            }
        }
    })
    .detach();
}

pub(crate) fn start_tray_handler_inner(
    i18n: &I18n,
    tx: async_channel::Sender<TrayEvent>,
) -> Option<TrayIcon> {
    match init_tray(i18n) {
        Ok((tray, show_id, quit_id)) => {
            tracing::info!("tray icon initialized successfully");

            #[cfg(not(target_os = "linux"))]
            spawn_tray_icon_event_forwarder(tx.clone());
            spawn_tray_menu_event_forwarder(tx, show_id, quit_id);

            Some(tray)
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to initialize tray icon");
            None
        }
    }
}

pub(crate) fn send_active_action(window_handle: WindowHandle<Root>, cx: &mut App) {
    window_handle
        .update(cx, |_, window: &mut gpui::Window, cx| {
            window.dispatch_action(Box::new(crate::gui::board::Active), cx);
        })
        .ok();
}

fn spawn_tray_menu_event_forwarder(
    tx: async_channel::Sender<TrayEvent>,
    show_id: MenuId,
    quit_id: MenuId,
) {
    let receiver = tray_icon::menu::MenuEvent::receiver().clone();
    super::utils::spawn_event_forwarder("tray-menu-event-forwarder", tx, move |forward| {
        while let Ok(event) = receiver.recv() {
            if !forward(tray_event_from_menu_event(&event, &show_id, &quit_id)) {
                break;
            }
        }
    });
}

#[cfg(not(target_os = "linux"))]
fn spawn_tray_icon_event_forwarder(tx: async_channel::Sender<TrayEvent>) {
    let receiver = TrayIconEvent::receiver().clone();
    super::utils::spawn_event_forwarder("tray-icon-event-forwarder", tx, move |forward| {
        while let Ok(event) = receiver.recv() {
            if !forward(tray_event_from_icon_event(&event)) {
                break;
            }
        }
    });
}

fn tray_event_from_menu_event(
    event: &tray_icon::menu::MenuEvent,
    show_id: &MenuId,
    quit_id: &MenuId,
) -> Option<TrayEvent> {
    if event.id == *show_id {
        Some(TrayEvent::Show)
    } else if event.id == *quit_id {
        Some(TrayEvent::Quit)
    } else {
        None
    }
}

#[cfg(not(target_os = "linux"))]
fn tray_event_from_icon_event(event: &TrayIconEvent) -> Option<TrayEvent> {
    if let TrayIconEvent::Click { button, .. } = event
        && *button == tray_icon::MouseButton::Left
    {
        Some(TrayEvent::Show)
    } else {
        None
    }
}

#[cfg(test)]
#[expect(clippy::panic)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[test]
    fn test_tray_state_default_has_no_icon() {
        let tray_state = TrayState::default();

        assert!(tray_state.tray_icon.is_none());
    }

    #[test]
    fn test_tray_state_rebuild_menu_without_icon_is_noop() {
        let tray_state = TrayState::default();
        let i18n = I18n::default();

        let result = tray_state.rebuild_menu(&i18n);

        assert!(result.is_ok());
        assert_eq!(result.ok(), Some(false));
    }

    #[test]
    fn test_load_tray_icon_rgba_resizes_to_fixed_square() {
        let Some(asset) = crate::gui::app::Assets::get("logo.png") else {
            panic!("embedded logo.png should exist");
        };

        let result = load_tray_icon_rgba(&asset.data);

        assert!(result.is_ok());

        let Ok((rgba, width, height)) = result else {
            unreachable!();
        };

        assert_eq!((width, height), (TRAY_ICON_SIZE, TRAY_ICON_SIZE));
        assert_eq!(rgba.len(), (width * height * 4) as usize);
    }

    #[test]
    fn test_icon_creation() {
        let icon = create_icon();
        assert!(icon.is_ok());
    }

    #[rstest]
    #[case(TRAY_SHOW_ID, Some(TrayEvent::Show))]
    #[case(TRAY_QUIT_ID, Some(TrayEvent::Quit))]
    #[case("other", None)]
    fn test_tray_event_from_menu_event_matches_menu_id(
        #[case] event_id: &str,
        #[case] expected: Option<TrayEvent>,
    ) {
        let show_id = MenuId::new(TRAY_SHOW_ID);
        let quit_id = MenuId::new(TRAY_QUIT_ID);
        let event = tray_icon::menu::MenuEvent {
            id: MenuId::new(event_id),
        };

        let result = tray_event_from_menu_event(&event, &show_id, &quit_id);

        assert!(matches!(
            (result, expected),
            (Some(TrayEvent::Show), Some(TrayEvent::Show))
                | (Some(TrayEvent::Quit), Some(TrayEvent::Quit))
                | (None, None)
        ));
    }

    #[cfg(not(target_os = "linux"))]
    #[rstest]
    #[case(
        TrayIconEvent::Click {
            id: tray_icon::TrayIconId::new("tray"),
            position: tray_icon::dpi::PhysicalPosition::new(0.0, 0.0),
            rect: tray_icon::Rect::default(),
            button: tray_icon::MouseButton::Left,
            button_state: tray_icon::MouseButtonState::Up,
        },
        Some(TrayEvent::Show)
    )]
    #[case(
        TrayIconEvent::Click {
            id: tray_icon::TrayIconId::new("tray"),
            position: tray_icon::dpi::PhysicalPosition::new(0.0, 0.0),
            rect: tray_icon::Rect::default(),
            button: tray_icon::MouseButton::Right,
            button_state: tray_icon::MouseButtonState::Up,
        },
        None
    )]
    #[case(
        TrayIconEvent::Enter {
            id: tray_icon::TrayIconId::new("tray"),
            position: tray_icon::dpi::PhysicalPosition::new(0.0, 0.0),
            rect: tray_icon::Rect::default(),
        },
        None
    )]
    fn test_tray_event_from_icon_event_filters_supported_actions(
        #[case] event: TrayIconEvent,
        #[case] expected: Option<TrayEvent>,
    ) {
        let result = tray_event_from_icon_event(&event);

        assert!(matches!(
            (result, expected),
            (Some(TrayEvent::Show), Some(TrayEvent::Show)) | (None, None)
        ));
    }
}

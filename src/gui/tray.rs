use gpui_kit::{App, AppContext, BorrowAppContext, Global, ReadGlobal, component::WindowExt};
use rust_i18n::t;
#[cfg(not(target_os = "linux"))]
use tray_icon::TrayIconEvent;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuId, MenuItem},
};

use crate::constants::APP_NAME;

/// Fixed menu IDs so event handlers remain valid after menu rebuilds.
const TRAY_SETTINGS_ID: &str = "tray_settings";
const TRAY_ABOUT_ID: &str = "tray_about";
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

    fn rebuild_menu(&self) -> Result<bool, Box<dyn std::error::Error>> {
        let Some(tray) = self.tray_icon.as_ref() else {
            return Ok(false);
        };

        let menu = build_tray_menu()?;
        tray.set_menu(Some(Box::new(menu)));
        Ok(true)
    }

    /// Install the platform tray handle into the global state.
    pub(crate) fn install(cx: &mut App, tray_icon: Option<TrayIcon>) {
        cx.update_global::<Self, _>(|state: &mut Self, _cx| {
            state.replace(tray_icon);
        });
    }

    pub(crate) fn refresh_menu(cx: &App) {
        Self::global(cx).refresh();
    }

    fn refresh(&self) {
        if let Err(error) = self.rebuild_menu() {
            tracing::warn!(%error, "failed to rebuild tray menu");
        }
    }
}

pub(crate) fn build_tray_menu() -> Result<Menu, Box<dyn std::error::Error>> {
    let settings_item = MenuItem::with_id(TRAY_SETTINGS_ID, t!("tray_settings"), true, None);
    let about_item = MenuItem::with_id(TRAY_ABOUT_ID, t!("tray_about"), true, None);
    let quit_item = MenuItem::with_id(TRAY_QUIT_ID, t!("tray_quit"), true, None);

    let tray_menu = Menu::new();
    tray_menu.append(&settings_item)?;
    tray_menu.append(&about_item)?;
    tray_menu.append(&quit_item)?;
    Ok(tray_menu)
}

pub(crate) fn init_tray() -> Result<(TrayIcon, MenuId, MenuId, MenuId), Box<dyn std::error::Error>>
{
    let tray_menu = build_tray_menu()?;
    let settings_id = MenuId::new(TRAY_SETTINGS_ID);
    let about_id = MenuId::new(TRAY_ABOUT_ID);
    let quit_id = MenuId::new(TRAY_QUIT_ID);

    let icon = create_icon()?;

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip(APP_NAME)
        .with_icon(icon)
        .with_menu_on_left_click(false)
        .build()?;

    Ok((tray, settings_id, about_id, quit_id))
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
    #[cfg(not(target_os = "linux"))]
    Show,
    Settings,
    About,
    Quit,
}

pub(crate) fn start_tray_handler(cx: &App) -> Option<TrayIcon> {
    let (tx, rx) = async_channel::unbounded();
    let tray = start_tray_handler_inner(tx);
    spawn_tray_event_loop(cx, rx);
    tray
}

fn spawn_tray_event_loop(cx: &App, rx: async_channel::Receiver<TrayEvent>) {
    cx.spawn(async move |async_app| {
        while let Ok(event) = rx.recv().await {
            match event {
                #[cfg(not(target_os = "linux"))]
                TrayEvent::Show => {
                    async_app.update(super::window::show);
                }
                TrayEvent::Settings => {
                    async_app.update(send_open_settings);
                }
                TrayEvent::About => {
                    async_app.update(send_open_about);
                }
                TrayEvent::Quit => {
                    async_app.update(|cx| cx.quit());
                }
            }
        }
    })
    .detach();
}

pub(crate) fn start_tray_handler_inner(tx: async_channel::Sender<TrayEvent>) -> Option<TrayIcon> {
    match init_tray() {
        Ok((tray, settings_id, about_id, quit_id)) => {
            tracing::info!("tray icon initialized successfully");

            #[cfg(not(target_os = "linux"))]
            spawn_tray_icon_event_forwarder(tx.clone());
            spawn_tray_menu_event_forwarder(tx, settings_id, about_id, quit_id);

            Some(tray)
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to initialize tray icon");
            None
        }
    }
}

pub(crate) fn send_open_settings(cx: &mut App) {
    open_panel(super::board::ActivePanel::Settings, cx);
}

pub(crate) fn send_open_about(cx: &mut App) {
    open_panel(super::board::ActivePanel::About, cx);
}

fn open_panel(panel: super::board::ActivePanel, cx: &mut App) {
    let Some(window_handle) = super::window::ensure_window(cx) else {
        return;
    };
    let _ = cx.update_window(window_handle.into(), |_, window, cx| {
        if let Some(board) = super::window::board(cx) {
            board.update(cx, |board, cx| {
                if window.has_active_dialog(cx) {
                    board.activate_popup(window, cx);
                    return;
                }
                board.cancel_shortcut_recording(cx);
                if board.settings_editor.hotkey.recording {
                    board.cancel_hotkey_recording(window, cx);
                }
                board.active_panel = panel;
                if panel == super::board::ActivePanel::Settings {
                    board
                        .settings_editor
                        .panel_state
                        .window_opacity_slider_visible = true;
                }
                board.activate_popup(window, cx);
            });
        }
    });
}

fn spawn_tray_menu_event_forwarder(
    tx: async_channel::Sender<TrayEvent>,
    settings_id: MenuId,
    about_id: MenuId,
    quit_id: MenuId,
) {
    let receiver = tray_icon::menu::MenuEvent::receiver().clone();
    super::utils::spawn_event_forwarder("tray-menu-event-forwarder", tx, move |forward| {
        while let Ok(event) = receiver.recv() {
            if !forward(tray_event_from_menu_event(
                &event,
                &settings_id,
                &about_id,
                &quit_id,
            )) {
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
    settings_id: &MenuId,
    about_id: &MenuId,
    quit_id: &MenuId,
) -> Option<TrayEvent> {
    if event.id == *settings_id {
        Some(TrayEvent::Settings)
    } else if event.id == *about_id {
        Some(TrayEvent::About)
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

    #[cfg(target_os = "linux")]
    #[test]
    #[ignore = "requires a private session bus; run with dbus-run-session"]
    fn test_tray_without_gtk_initializes_refreshes_and_drops() {
        let (tray, ..) = init_tray().unwrap_or_else(|error| panic!("tray init: {error}"));
        let mut state = TrayState::default();
        state.replace(Some(tray));

        futures_lite::future::block_on(assert_dbus_tray_menu());
        assert_eq!(state.rebuild_menu().ok(), Some(true));
        futures_lite::future::block_on(assert_dbus_tray_menu());
        state.replace(None);
        assert_eq!(state.rebuild_menu().ok(), Some(false));
    }

    #[cfg(target_os = "linux")]
    async fn assert_dbus_tray_menu() {
        verify_dbus_tray_menu()
            .await
            .unwrap_or_else(|error| panic!("D-Bus tray menu: {error}"));
    }

    #[cfg(target_os = "linux")]
    async fn verify_dbus_tray_menu() -> Result<(), Box<dyn std::error::Error>> {
        use std::{collections::HashMap, time::Duration};

        use ashpd::zbus::{
            self,
            zvariant::{OwnedObjectPath, OwnedValue, Value},
        };

        let connection = zbus::Connection::session().await?;
        let bus = zbus::fdo::DBusProxy::new(&connection).await?;
        let prefix = format!("org.kde.StatusNotifierItem-{}-", std::process::id());
        let service = bus
            .list_names()
            .await?
            .into_iter()
            .find(|name| name.starts_with(&prefix))
            .ok_or("tray service was not registered")?;
        let tray = zbus::Proxy::new(
            &connection,
            service.to_string(),
            "/StatusNotifierItem",
            "org.kde.StatusNotifierItem",
        )
        .await?;
        assert_eq!(tray.get_property::<String>("Status").await?, "Active");
        let icons: Vec<(i32, i32, Vec<u8>)> = tray.get_property("IconPixmap").await?;
        assert_eq!((icons[0].0, icons[0].1), (32, 32));
        let path: OwnedObjectPath = tray.get_property("Menu").await?;
        let menu = zbus::Proxy::new(
            &connection,
            service.to_string(),
            path,
            "com.canonical.dbusmenu",
        )
        .await?;
        let items: Vec<(i32, HashMap<String, OwnedValue>)> = menu
            .call("GetGroupProperties", &(Vec::<i32>::new(), vec!["label"]))
            .await?;
        let items: Vec<_> = items.iter().filter(|(id, _)| *id != 0).collect();
        assert_eq!(items.len(), 3);
        let receiver = tray_icon::menu::MenuEvent::receiver();
        for ((id, properties), (label, expected_id)) in items.iter().zip([
            (t!("tray_settings"), TRAY_SETTINGS_ID),
            (t!("tray_about"), TRAY_ABOUT_ID),
            (t!("tray_quit"), TRAY_QUIT_ID),
        ]) {
            assert_eq!(<&str>::try_from(&properties["label"])?, label.as_ref());
            menu.call::<_, _, ()>("Event", &(*id, "clicked", Value::from(0_i32), 0_u32))
                .await?;
            assert_eq!(
                receiver.recv_timeout(Duration::from_secs(2))?.id,
                MenuId::new(expected_id)
            );
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_tray_menu_without_gtk_preserves_action_ids() {
        let menu = build_tray_menu().unwrap_or_else(|error| panic!("tray menu: {error}"));
        let ids: Vec<_> = menu.items().iter().map(|item| item.id().clone()).collect();

        assert_eq!(
            ids,
            [TRAY_SETTINGS_ID, TRAY_ABOUT_ID, TRAY_QUIT_ID].map(MenuId::new)
        );
    }

    #[test]
    fn test_tray_state_default_has_no_icon() {
        let tray_state = TrayState::default();

        assert!(tray_state.tray_icon.is_none());
    }

    #[test]
    fn test_tray_state_rebuild_menu_without_icon_is_noop() {
        let tray_state = TrayState::default();

        let result = tray_state.rebuild_menu();

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
    #[case(TRAY_SETTINGS_ID, Some(TrayEvent::Settings))]
    #[case(TRAY_ABOUT_ID, Some(TrayEvent::About))]
    #[case(TRAY_QUIT_ID, Some(TrayEvent::Quit))]
    #[case("other", None)]
    fn test_tray_event_from_menu_event_matches_menu_id(
        #[case] event_id: &str,
        #[case] expected: Option<TrayEvent>,
    ) {
        let settings_id = MenuId::new(TRAY_SETTINGS_ID);
        let about_id = MenuId::new(TRAY_ABOUT_ID);
        let quit_id = MenuId::new(TRAY_QUIT_ID);
        let event = tray_icon::menu::MenuEvent {
            id: MenuId::new(event_id),
        };

        let result = tray_event_from_menu_event(&event, &settings_id, &about_id, &quit_id);

        assert_eq!(result, expected);
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

        assert_eq!(result, expected);
    }
}

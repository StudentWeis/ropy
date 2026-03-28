use std::{
    sync::{mpsc, mpsc::Sender},
    time::Duration,
};

#[cfg(target_os = "linux")]
use gpui::AppContext;
use gpui::{App, BackgroundExecutor, WindowHandle};
use gpui_component::Root;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuId, MenuItem},
};

use crate::{constants::APP_NAME, i18n::I18n};

/// Fixed menu IDs so event handlers remain valid after menu rebuilds.
const TRAY_SHOW_ID: &str = "tray_show";
const TRAY_QUIT_ID: &str = "tray_quit";
const TRAY_ICON_SIZE: u32 = 32;

/// Build a tray menu with translated labels.
pub fn build_tray_menu(i18n: &I18n) -> Result<Menu, Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(TRAY_SHOW_ID, i18n.t("tray_show"), true, None);
    let quit_item = MenuItem::with_id(TRAY_QUIT_ID, i18n.t("tray_quit"), true, None);

    let tray_menu = Menu::new();
    tray_menu.append(&show_item)?;
    tray_menu.append(&quit_item)?;
    Ok(tray_menu)
}

/// Initialize and return the tray icon
pub fn init_tray(i18n: &I18n) -> Result<(TrayIcon, MenuId, MenuId), Box<dyn std::error::Error>> {
    let tray_menu = build_tray_menu(i18n)?;
    let show_id = MenuId::new(TRAY_SHOW_ID);
    let quit_id = MenuId::new(TRAY_QUIT_ID);

    let icon = create_icon()?;

    // Create tray icon
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip(APP_NAME)
        .with_icon(icon)
        .with_menu_on_left_click(false)
        .build()?;

    Ok((tray, show_id, quit_id))
}

/// Create a simple icon for the tray
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

pub enum TrayEvent {
    Show,
    Quit,
}

pub fn start_tray_handler(
    i18n: &I18n,
    cx: &App,
    window_handle: WindowHandle<Root>,
) -> Option<TrayIcon> {
    let (tx, rx) = mpsc::channel();

    let bg_executor = cx.background_executor().clone();

    #[cfg(not(target_os = "linux"))]
    let tray = start_tray_handler_inner(i18n, tx, &bg_executor);

    #[cfg(target_os = "linux")]
    let tray = {
        let i18n = i18n.clone();
        let bg_executor_clone = bg_executor.clone();
        // On Linux, tray must be initialized on the GTK thread.
        // We cannot return the TrayIcon from the spawned task, so we
        // leak it there and return None to the caller.
        cx.background_spawn(async move {
            gtk::init().expect("Failed to init gtk modules");
            if let Some(tray) = start_tray_handler_inner(&i18n, tx, &bg_executor_clone) {
                Box::leak(Box::new(tray));
            }
            gtk::main();
        })
        .detach();
        None
    };

    cx.spawn(async move |async_app: &mut gpui::AsyncApp| {
        let bg_executor = async_app.background_executor().clone();
        loop {
            while let Ok(event) = rx.try_recv() {
                match event {
                    TrayEvent::Show => {
                        let _ = async_app.update(move |cx| {
                            crate::gui::tray::send_active_action(window_handle, cx);
                        });
                    }
                    TrayEvent::Quit => {
                        let _ = async_app.update(move |cx: &mut gpui::App| {
                            cx.quit();
                        });
                    }
                }
            }

            bg_executor.timer(Duration::from_millis(100)).await;
        }
    })
    .detach();

    tray
}

/// Start the system tray handler
pub fn start_tray_handler_inner(
    i18n: &I18n,
    tx: Sender<TrayEvent>,
    bg_executor: &BackgroundExecutor,
) -> Option<TrayIcon> {
    match init_tray(i18n) {
        Ok((tray, show_id, quit_id)) => {
            tracing::info!("tray icon initialized successfully");

            let bg_executor_clone = bg_executor.clone();

            bg_executor
                .spawn(async move {
                    let menu_channel = tray_icon::menu::MenuEvent::receiver();
                    let tray_channel = TrayIconEvent::receiver();

                    loop {
                        while let Ok(event) = menu_channel.try_recv() {
                            if event.id == show_id {
                                let _ = tx.send(TrayEvent::Show);
                            } else if event.id == quit_id {
                                let _ = tx.send(TrayEvent::Quit);
                            }
                        }

                        while let Ok(event) = tray_channel.try_recv() {
                            if let TrayIconEvent::Click { button, .. } = event
                                && button == tray_icon::MouseButton::Left
                            {
                                let _ = tx.send(TrayEvent::Show);
                            }
                        }

                        bg_executor_clone.timer(Duration::from_millis(100)).await;
                    }
                })
                .detach();

            Some(tray)
        }
        Err(e) => {
            tracing::error!(error = %e, "failed to initialize tray icon");
            None
        }
    }
}

/// Send the active action to the main window
pub fn send_active_action(window_handle: WindowHandle<Root>, cx: &mut gpui::App) {
    window_handle
        .update(cx, |_, window: &mut gpui::Window, cx| {
            window.dispatch_action(Box::new(crate::gui::board::Active), cx);
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

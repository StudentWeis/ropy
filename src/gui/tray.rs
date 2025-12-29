use crate::config::Settings;
use crate::i18n::I18n;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;

use gpui::{AsyncApp, WindowHandle};
use gpui_component::Root;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuId, MenuItem},
};

/// Initialize and return the tray icon
pub fn init_tray(
    settings: Arc<RwLock<Settings>>,
) -> Result<(TrayIcon, MenuId, MenuId), Box<dyn std::error::Error>> {
    let language = settings.read().unwrap().language;
    let i18n = I18n::new(language).unwrap_or_default();

    // Create menu items
    let show_item = MenuItem::new(i18n.t("tray_show"), true, None);
    let quit_item = MenuItem::new(i18n.t("tray_quit"), true, None);

    // Create menu
    let tray_menu = Menu::new();
    tray_menu.append(&show_item)?;
    tray_menu.append(&quit_item)?;

    let icon = create_icon()?;

    // Create tray icon
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip(i18n.t("app_name").as_str())
        .with_icon(icon)
        .with_menu_on_left_click(false)
        .build()?;

    Ok((tray, show_item.id().clone(), quit_item.id().clone()))
}

/// Create a simple icon for the tray
fn create_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let img = image::open("assets/logo.png")?;
    let rgba = img.to_rgba8().into_raw();
    let width = img.width();
    let height = img.height();
    Icon::from_rgba(rgba, width, height).map_err(|e| format!("Failed to create icon: {e:?}").into())
}

/// Start the system tray handler
pub fn start_tray_handler(
    window_handle: WindowHandle<Root>,
    async_app: AsyncApp,
    settings: Arc<RwLock<Settings>>,
) {
    let fg_executor = async_app.foreground_executor().clone();
    let bg_executor = async_app.background_executor().clone();
    match init_tray(settings) {
        Ok((tray, show_id, quit_id)) => {
            println!("[ropy] Tray icon initialized successfully");
            // Keep tray icon alive for the lifetime of the application
            Box::leak(Box::new(tray));
            fg_executor
                .spawn(async move {
                    let menu_channel = tray_icon::menu::MenuEvent::receiver();
                    let tray_channel = TrayIconEvent::receiver();
                    loop {
                        while let Ok(event) = menu_channel.try_recv() {
                            if event.id == show_id {
                                let _ = async_app.update(move |cx| {
                                    window_handle
                                        .update(cx, |_, window, cx| {
                                            window.dispatch_action(
                                                Box::new(crate::gui::board::Active),
                                                cx,
                                            )
                                        })
                                        .ok();
                                });
                            } else if event.id == quit_id {
                                let _ = async_app.update(move |cx| {
                                    cx.quit();
                                });
                            }
                        }
                        while let Ok(event) = tray_channel.try_recv() {
                            if let TrayIconEvent::Click { button, .. } = event
                                && button == tray_icon::MouseButton::Left
                            {
                                let _ = async_app.update(move |cx| {
                                    window_handle
                                        .update(cx, |_, window, cx| {
                                            window.dispatch_action(
                                                Box::new(crate::gui::board::Active),
                                                cx,
                                            )
                                        })
                                        .ok();
                                });
                            }
                        }
                        bg_executor.timer(Duration::from_millis(100)).await;
                    }
                })
                .detach();
        }
        Err(e) => {
            eprintln!("[ropy] Failed to initialize tray icon: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_creation() {
        let icon = create_icon();
        assert!(icon.is_ok());
    }
}

use std::sync::mpsc::Sender;
use tray_icon::{
    Icon, TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem},
};

/// Tray icon events
#[derive(Debug, Clone, Copy)]
pub enum TrayEvent {
    /// Show the main window
    Show,
    /// Quit the application
    Quit,
}

/// Initialize and return the tray icon
pub fn init_tray(tx: Sender<TrayEvent>) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    // Create menu items
    let show_item = MenuItem::new("Show", true, None);
    let quit_item = MenuItem::new("Quit", true, None);

    // Create menu
    let tray_menu = Menu::new();
    tray_menu.append(&show_item)?;
    tray_menu.append(&quit_item)?;

    let icon = create_icon()?;

    // Create tray icon
    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(tray_menu))
        .with_tooltip("Ropy")
        .with_icon(icon)
        .build()?;

    // Handle menu events
    let menu_channel = MenuEvent::receiver();
    let show_id = show_item.id().clone();
    let quit_id = quit_item.id().clone();

    std::thread::spawn(move || {
        loop {
            if let Ok(event) = menu_channel.recv() {
                if event.id == show_id {
                    let _ = tx.send(TrayEvent::Show);
                } else if event.id == quit_id {
                    let _ = tx.send(TrayEvent::Quit);
                }
            }
        }
    });

    Ok(tray)
}

/// Create a simple icon for the tray
fn create_icon() -> Result<Icon, Box<dyn std::error::Error>> {
    let img_data = include_bytes!("../../assets/logo.png");
    let img = image::load_from_memory(img_data)?;
    let rgba = img.to_rgba8().into_raw();
    let width = img.width();
    let height = img.height();
    Icon::from_rgba(rgba, width, height)
        .map_err(|e| format!("Failed to create icon: {:?}", e).into())
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

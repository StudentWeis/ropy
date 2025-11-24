mod gui;
mod monitor;

#[cfg(debug_assertions)]
use std::time::Duration;

fn main() {
    #[cfg(debug_assertions)]
    let _monitor_handle = monitor::spawn_rss_monitor(Duration::from_secs(1));

    gui::gpui::gpui_test();
}

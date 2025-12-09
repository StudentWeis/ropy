#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod gui;
mod monitor;
mod repository;

#[cfg(target_os = "windows")]
mod single_instance;

fn main() {
    // Ensure single instance on Windows
    #[cfg(target_os = "windows")]
    if !single_instance::ensure_single_instance() {
        return;
    }

    // Monitor RSS in debug mode
    #[cfg(debug_assertions)]
    let _monitor_handle = monitor::spawn_rss_monitor(std::time::Duration::from_secs(2));

    gui::launch_app();
}

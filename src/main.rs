#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod config;
mod gui;
mod i18n;
mod repository;
mod utils;

fn main() {
    // Ensure single instance on Windows
    #[cfg(target_os = "windows")]
    if !utils::ensure_single_instance() {
        return;
    }

    gui::launch_app();
}

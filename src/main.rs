#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod config;
mod gui;
mod i18n;
mod repository;

#[cfg(target_os = "windows")]
mod single_instance;

fn main() {
    // Ensure single instance on Windows
    #[cfg(target_os = "windows")]
    if !single_instance::ensure_single_instance() {
        return;
    }

    gui::launch_app();
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod config;
mod constants;
mod gui;
mod i18n;
mod repository;
mod updater;
mod utils;

fn main() {
    let _logging_guard = utils::init_logging();

    // Ensure single instance on Windows
    #[cfg(target_os = "windows")]
    if !utils::ensure_single_instance() {
        return;
    }

    gui::launch_app();
}

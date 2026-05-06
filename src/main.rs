//! Ropy - A clipboard manager built with Rust and GPUI.

// Configure the application to run without a console window on Windows in release mode
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Application lifecycle orchestration.
pub mod app;
/// Clipboard capture, normalization, and write-back.
pub mod clipboard;
/// User configuration loading and persistence.
pub mod config;
/// Shared application constants.
pub mod constants;
/// GPUI windowing, rendering, and interaction.
pub mod gui;
/// Internationalization data and runtime translation helpers.
pub mod i18n;
/// Persistent clipboard storage and query logic.
pub mod repository;
/// Update checking and installation flows.
pub mod updater;
/// Cross-cutting utility helpers.
pub mod utils;

fn main() {
    let _logging_guard = utils::init_logging();

    // Ensure single instance on Windows
    #[cfg(target_os = "windows")]
    if !utils::ensure_single_instance() {
        return;
    }

    app::launch();
}

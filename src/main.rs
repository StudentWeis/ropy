//! Ropy - A clipboard manager built with Rust and GPUI.

// Configure the application to run without a console window on Windows in release mode
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Isolated developer benchmark entry point.
pub mod benchmark;

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
pub use ropy_core::repository;
/// Update checking and installation flows.
pub mod updater;
/// Cross-cutting utility helpers.
pub mod utils;

/// Isolated promotional renders and lossless screenshot composition.
pub mod promo;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Some(ready) = benchmark::parse(&std::env::args_os().skip(1).collect::<Vec<_>>())? {
        benchmark::run(ready)?;
        return Ok(());
    }

    if let Some(command) =
        promo::PromoCommand::parse(&std::env::args_os().skip(1).collect::<Vec<_>>())?
    {
        command.run()?;
        return Ok(());
    }

    if updater::transaction::handle_startup()? {
        return Ok(());
    }

    let _logging_guard = utils::init_logging();

    // Ensure single instance on Windows
    #[cfg(target_os = "windows")]
    if !utils::ensure_single_instance() {
        return Ok(());
    }

    app::launch();
    Ok(())
}

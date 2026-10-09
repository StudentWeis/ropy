//! Portable settings and stable serialized identifiers.

/// Locale codes stored in configuration.
pub mod language_id;
/// Settings format, persistence and platform-independent validation.
pub mod settings;
/// Theme codes and legacy aliases.
pub mod theme_id;

pub use language_id::Language;
pub use settings::{ConfirmMode, LayoutMode, Settings, SettingsError, WindowSettings};
pub use theme_id::ThemeId;

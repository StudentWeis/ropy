/// Platform-specific auto-start integration.
pub mod autostart;
/// Persisted locale codes.
pub mod language_id;
/// Persisted user settings and validation.
pub mod settings;
/// Persisted theme codes and legacy aliases.
pub mod theme_id;

pub(crate) use autostart::{AutoStartError, AutoStartManager};
pub(crate) use settings::{ConfirmMode, LayoutMode, Settings, WindowSettings};

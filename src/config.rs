/// Platform-specific auto-start integration.
pub mod autostart;
/// Persisted user settings and validation.
pub mod settings;

pub(crate) use autostart::{AutoStartError, AutoStartManager};
pub(crate) use settings::{ConfirmMode, LayoutMode, Settings, WindowSettings};

/// Platform-specific auto-start integration.
pub mod autostart;

pub(crate) use autostart::{AutoStartError, AutoStartManager};
pub(crate) use ropy_core::config::{
    ConfirmMode, LayoutMode, Settings, WindowSettings, language_id, theme_id,
};

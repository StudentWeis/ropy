//! Cross-module constants whose values are not user-localizable.

/// Display name shown in tray tooltips, window titles, the About panel,
/// etc. Intentionally not in the i18n bundles — the product name stays
/// "Ropy" in every locale.
pub(crate) const APP_NAME: &str = "Ropy";

/// Hidden CLI flag used by the auto-start launcher to suppress the
/// initial window so the app boots silently into the tray.
pub(crate) const SILENT_ARG: &str = "--silent";

pub(crate) const BACK_ARROW: &str = "←";

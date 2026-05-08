//! Cross-module constants whose values are not user-localizable.

/// Display name shown in tray tooltips, window titles, the About panel,
/// etc. Intentionally not in the i18n bundles — the product name stays
/// "Ropy" in every locale.
pub(crate) const APP_NAME: &str = "Ropy";

/// Hidden CLI flag historically used by the auto-start launcher to
/// suppress the initial window. Ropy now never opens the window on
/// launch — every entry point boots straight into the tray and the
/// window only appears when the user invokes the global hotkey or a
/// tray-menu action — so this flag is effectively a no-op.
///
/// It is still passed by `auto-launch` registrations because existing
/// `LaunchAgents` on users' machines already reference it; rejecting or
/// removing the arg would force a re-registration on every install.
pub(crate) const SILENT_ARG: &str = "--silent";

pub(crate) const BACK_ARROW: &str = "←";

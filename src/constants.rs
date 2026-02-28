#![allow(clippy::empty_line_after_doc_comments)]

/// Application-wide constants used throughout the codebase.
/// The only public constant defined today is the display name of the
/// application.  Historically this string lived in the i18n translation
/// files, but it never needs localisation – the name of the app is always
/// "Ropy" regardless of UI language.  Keeping it in a central constant
/// reduces duplication and simplifies the translation files.

/// Name presented to the user in tooltips, window titles, about panel, etc.
pub const APP_NAME: &str = "Ropy";

/// Arrow used for the "back" button in the about panel.
pub const ABOUT_BACK_ARROW: &str = "←";

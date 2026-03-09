use thiserror::Error;

/// I18n-related errors.
#[derive(Debug, Error)]
pub enum I18nError {
    #[error("Locale file not found: {0}")]
    NotFound(String),
    #[error("Failed to parse translation file: {0}")]
    ParseError(String),
}

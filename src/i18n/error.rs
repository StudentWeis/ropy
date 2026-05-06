use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum I18nError {
    #[error("Locale file not found: {0}")]
    NotFound(String),
    #[error("Failed to parse translation file: {0}")]
    ParseError(String),
}

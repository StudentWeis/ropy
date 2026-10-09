//! Clipboard history persistence and portable configuration for Ropy.
//!
//! This library has no desktop runtime or native clipboard dependency. The
//! application owns scheduling, platform shortcut validation and presentation.

/// Persisted application settings and identifiers.
pub mod config;
/// Clipboard records, payload files and transactional storage.
pub mod repository;

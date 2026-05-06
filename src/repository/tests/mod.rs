#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Repository integration tests, split by theme.
//!
//! Each module focuses on one behavioral concern so a single test edit only
//! recompiles its own file rather than the whole `repo.rs` translation unit.

mod dedup_tests;
mod display_tests;
mod pin_tests;
mod save_tests;

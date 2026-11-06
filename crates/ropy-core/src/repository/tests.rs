#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
//! Repository integration tests, split by theme.
//!
//! Each module groups one behavioral concern to make tests easier to navigate.
//! These modules still compile together as part of the crate's test target.

mod dedup_tests;
mod display_tests;
mod pin_tests;
mod recovery_tests;
mod save_tests;

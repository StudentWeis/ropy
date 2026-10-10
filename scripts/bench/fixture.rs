//! Shared v1 benchmark fixture: 200 unique ASCII records of exactly 1 KiB.

use ropy_core::repository::{ClipboardRepository, errors::RepositoryError};

pub(super) fn text(index: usize) -> String {
    let prefix = format!("ropy-bench-v1-{index:08}-");
    format!("{prefix}{}", "x".repeat(1024 - prefix.len()))
}

pub(super) fn seed(repository: &ClipboardRepository) -> Result<(), RepositoryError> {
    for index in 0..200 {
        repository.save_text(text(index))?;
    }
    Ok(())
}

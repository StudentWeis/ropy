use std::sync::Arc;

use gpui::{App, Global, ReadGlobal};

/// Storage backend traits and implementations (memory, redb).
pub mod backend;
mod cleanup;
mod display;
/// Repository error types.
pub mod errors;
mod favorites;
/// Persisted clipboard record models.
pub mod models;
/// Repository entry points and persistence APIs.
pub mod repo;
mod sidecar;
#[cfg(test)]
mod test_helpers;
#[cfg(test)]
mod tests;
mod time_index;

pub(crate) use models::{ClipboardRecord, ContentType, RichTextMeta, SharedRecords};
pub(crate) use repo::ClipboardRepository;

/// GPUI Global wrapper for the clipboard repository.
///
/// Registered once at startup so any component with access to `&App` can
/// retrieve the shared repository without parameter threading.
#[derive(Clone)]
pub(crate) struct GlobalRepository(Option<Arc<ClipboardRepository>>);

impl Global for GlobalRepository {}

impl GlobalRepository {
    pub(crate) const fn new(repository: Option<Arc<ClipboardRepository>>) -> Self {
        Self(repository)
    }

    /// Get a reference to the inner repository, if available.
    pub(crate) const fn get(&self) -> Option<&Arc<ClipboardRepository>> {
        self.0.as_ref()
    }

    /// Clone the inner `Arc<ClipboardRepository>`, if available.
    pub(crate) fn cloned(&self) -> Option<Arc<ClipboardRepository>> {
        self.0.clone()
    }

    /// Read the global repository via a closure.
    pub(crate) fn read<R>(
        cx: &App,
        reader: impl FnOnce(Option<&Arc<ClipboardRepository>>) -> R,
    ) -> R {
        reader(Self::global(cx).get())
    }
}

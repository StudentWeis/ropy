use std::sync::Arc;

use gpui::{App, Global, ReadGlobal};

pub mod errors;
pub mod models;
pub mod repo;
mod time_index;

pub use models::{ClipboardRecord, ContentType, SharedRecords};
pub use repo::ClipboardRepository;

/// GPUI Global wrapper for the clipboard repository.
///
/// Registered once at startup so any component with access to `&App` can
/// retrieve the shared repository without parameter threading.
#[derive(Clone)]
pub struct GlobalRepository(Option<Arc<ClipboardRepository>>);

impl Global for GlobalRepository {}

impl GlobalRepository {
    pub const fn new(repository: Option<Arc<ClipboardRepository>>) -> Self {
        Self(repository)
    }

    /// Get a reference to the inner repository, if available.
    pub const fn get(&self) -> Option<&Arc<ClipboardRepository>> {
        self.0.as_ref()
    }

    /// Clone the inner `Arc<ClipboardRepository>`, if available.
    pub fn cloned(&self) -> Option<Arc<ClipboardRepository>> {
        self.0.clone()
    }

    /// Read the global repository via a closure.
    pub fn read<R>(cx: &App, reader: impl FnOnce(Option<&Arc<ClipboardRepository>>) -> R) -> R {
        reader(Self::global(cx).get())
    }
}

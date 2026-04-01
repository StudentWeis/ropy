#[cfg(feature = "dhat-heap")]
use std::{
    path::Path,
    sync::{Mutex, MutexGuard, OnceLock},
};

#[cfg(feature = "dhat-heap")]
static HEAP_PROFILER: OnceLock<Mutex<Option<dhat::Profiler>>> = OnceLock::new();

#[cfg(feature = "dhat-heap")]
pub const DHAT_OUTPUT_PATH: &str = "target/dhat-heap.json";

#[cfg(feature = "dhat-heap")]
fn heap_profiler() -> &'static Mutex<Option<dhat::Profiler>> {
    HEAP_PROFILER.get_or_init(|| Mutex::new(None))
}

#[cfg(feature = "dhat-heap")]
fn lock_heap_profiler() -> MutexGuard<'static, Option<dhat::Profiler>> {
    heap_profiler()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(feature = "dhat-heap")]
pub fn start_heap_profiling() {
    let started = {
        let mut guard = lock_heap_profiler();
        if guard.is_some() {
            false
        } else {
            if let Some(parent) = Path::new(DHAT_OUTPUT_PATH).parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            *guard = Some(
                dhat::Profiler::builder()
                    .file_name(DHAT_OUTPUT_PATH)
                    .build(),
            );
            true
        }
    };

    if started {
        tracing::info!(
            dhat_output_path = DHAT_OUTPUT_PATH,
            "dhat heap profiling enabled; profile will be flushed before app quit"
        );
    }
}

#[cfg(not(feature = "dhat-heap"))]
pub const fn start_heap_profiling() {}

#[cfg(feature = "dhat-heap")]
pub fn finish_heap_profiling() {
    let profiler = {
        let mut guard = lock_heap_profiler();
        guard.take()
    };

    if profiler.is_some() {
        tracing::info!(
            dhat_output_path = DHAT_OUTPUT_PATH,
            "flushing dhat heap profile before application exit"
        );
    }

    drop(profiler);
}

#[cfg(not(feature = "dhat-heap"))]
pub const fn finish_heap_profiling() {}

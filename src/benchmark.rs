//! Isolated hidden desktop for the v1 RSS baseline; no OS integrations are started.

use std::{
    ffi::OsString,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};

use thiserror::Error;

use crate::{
    config::Settings,
    gui::{repository::GlobalRepository, settings::GlobalSettings},
    i18n::I18n,
    repository::ClipboardRepository,
};

#[path = "../scripts/bench/fixture.rs"]
mod fixture;

#[derive(Debug, Error)]
pub(crate) enum BenchmarkError {
    #[error("usage: --bench-memory READY_FILE")]
    Arguments,
    #[error("benchmark I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("benchmark repository failed: {0}")]
    Repository(#[from] crate::repository::errors::RepositoryError),
}

pub(crate) fn parse(args: &[OsString]) -> Result<Option<PathBuf>, BenchmarkError> {
    if args.first().is_none_or(|arg| arg != "--bench-memory") {
        return Ok(None);
    }
    let [_, ready] = args else {
        return Err(BenchmarkError::Arguments);
    };
    Ok(Some(ready.into()))
}

pub(crate) fn run(ready: PathBuf) -> Result<(), BenchmarkError> {
    // Require a fresh directory beside the readiness file. The runner owns its
    // lifetime, including cleanup after terminating the GUI process.
    let directory = ready
        .parent()
        .ok_or(BenchmarkError::Arguments)?
        .join("data");
    std::fs::create_dir(&directory)?;
    let repository =
        ClipboardRepository::open(&directory.join("clipboard.redb"), directory.join("images"))?;
    fixture::seed(&repository)?;
    let records = repository.get_display_records(100)?;
    let readiness = format!(
        "{{\"stored_records\":{},\"loaded_records\":{}}}",
        repository.count(),
        records.len()
    );
    let repository = Arc::new(repository);
    gpui_kit::application()
        .with_assets(crate::gui::Assets)
        .run(move |cx| {
            #[cfg(target_os = "macos")]
            crate::gui::set_activation_policy_accessory();
            gpui_kit::init(cx);
            let mut settings = Settings::default();
            settings.autostart.enabled = false;
            settings.update.auto_check = false;
            settings.storage.max_history_records = 100;
            settings.storage.max_storage_records = 200;
            cx.set_global(I18n::load_i18n(settings.language.clone()));
            cx.set_global(GlobalSettings::new(settings));
            cx.set_global(GlobalRepository::new(Some(repository)));
            let (copy_tx, _copy_rx) = async_channel::unbounded();
            crate::gui::create_window(
                cx,
                Arc::new(RwLock::new(records)),
                Arc::new(Mutex::default()),
                copy_tx,
            );
            // Publish atomically only after the real hidden board has been created.
            let pending = ready.with_extension("pending");
            if std::fs::write(&pending, readiness)
                .and_then(|()| std::fs::rename(pending, &ready))
                .is_err()
            {
                cx.quit();
            }
        });
    Ok(())
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_incomplete_benchmark_rejects_normal_startup() {
        assert!(parse(&["--bench-memory".into()]).is_err());
        assert!(parse(&["--bench-memory".into(), "ready".into(), "extra".into()]).is_err());
        assert!(parse(&[]).unwrap().is_none());
    }

    #[test]
    fn test_fixture_seed_and_recopy_preserve_expected_counts() {
        let temporary = tempfile::tempdir().unwrap();
        let repository = ClipboardRepository::open(
            &temporary.path().join("clipboard.redb"),
            temporary.path().join("images"),
        )
        .unwrap();
        fixture::seed(&repository).unwrap();
        assert_eq!(repository.count(), 200);
        assert_eq!(repository.get_display_records(100).unwrap().len(), 100);
        for index in 0..200 {
            assert_eq!(fixture::text(index).len(), 1024);
        }
        repository.save_text(fixture::text(0)).unwrap();
        assert_eq!(repository.count(), 200);
        repository.save_text(fixture::text(200)).unwrap();
        assert_eq!(repository.count(), 201);
    }
}

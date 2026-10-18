//! Atomic theme replacement and a prepared rollback, with no GUI dependency.
use super::{
    ThemeDefinition, ThemeId,
    document::{DocumentError, ThemeDocument},
};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use tempfile::NamedTempFile;

#[derive(Debug, thiserror::Error)]
#[expect(clippy::redundant_pub_crate)]
pub(crate) enum ThemeWriteError {
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("the theme file changed outside the editor")]
    Changed,
    #[error("could not restore the theme; recovery copy: {backup}: {source}")]
    Restore {
        backup: PathBuf,
        source: std::io::Error,
    },
}

#[expect(clippy::redundant_pub_crate)]
pub(crate) struct ThemeWrite {
    pub(crate) definition: ThemeDefinition,
    pub(crate) source: String,
    path: PathBuf,
    backup: Option<NamedTempFile>,
}

impl ThemeWrite {
    /// Prepare the old file before replacing it, so rollback never truncates it.
    pub(crate) fn save(
        directory: &Path,
        document: &ThemeDocument,
        existing: Option<(&ThemeId, &str)>,
    ) -> Result<Self, ThemeWriteError> {
        let source = document.serialize()?;
        std::fs::create_dir_all(directory)?;
        let file = staged_file(directory, source.as_bytes())?;
        let id = existing.map_or_else(
            || {
                let suffix = file
                    .path()
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase();
                ThemeId::new(format!(
                    "user:custom-{}",
                    suffix.trim_start_matches(".theme-")
                ))
            },
            |(id, _)| id.clone(),
        );
        let path = directory.join(format!("{}.toml", id.code().trim_start_matches("user:")));
        let backup = if let Some((_, expected)) = existing {
            let previous = std::fs::read(&path)?;
            if previous != expected.as_bytes() {
                return Err(ThemeWriteError::Changed);
            }
            Some(staged_file(directory, &previous)?)
        } else {
            None
        };
        if existing.is_some() {
            file.persist(&path).map_err(|error| error.error)?;
        } else {
            file.persist_noclobber(&path).map_err(|error| error.error)?;
        }
        Ok(Self {
            definition: document.definition(id),
            source,
            path,
            backup,
        })
    }

    pub(crate) fn rollback(self) -> Result<(), ThemeWriteError> {
        if let Some(backup) = self.backup {
            if let Err(error) = backup.persist(&self.path) {
                // PersistError retains the file. Disable automatic cleanup so the
                // original bytes survive even when restoring the destination fails.
                let source = error.error;
                let backup = error
                    .file
                    .into_temp_path()
                    .keep()
                    .map_err(|error| error.error)?;
                return Err(ThemeWriteError::Restore { backup, source });
            }
        } else {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}

fn staged_file(directory: &Path, bytes: &[u8]) -> Result<NamedTempFile, std::io::Error> {
    // A staging file can never be a selectable .toml file or its own destination.
    let mut file = tempfile::Builder::new()
        .prefix(".theme-")
        .suffix(".tmp")
        .rand_bytes(12)
        .tempfile_in(directory)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    Ok(file)
}

#[expect(clippy::redundant_pub_crate)]
pub(crate) fn export(path: &Path, source: &str) -> Result<(), std::io::Error> {
    let directory = path.parent().unwrap_or_else(|| Path::new("."));
    staged_file(directory, source.as_bytes())?
        .persist(path)
        .map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]
    use super::*;

    fn document() -> ThemeDocument {
        ThemeDocument::from(&ThemeDefinition::load(&ThemeId::new("ropy-light")).unwrap())
    }

    #[test]
    fn test_theme_write_new_file_is_loadable_and_rollback_removes_it() {
        let directory = tempfile::tempdir().unwrap();
        let write = ThemeWrite::save(directory.path(), &document(), None).unwrap();
        assert_ne!(write.path.extension().unwrap(), "tmp");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        let loaded = super::super::ThemeCatalog::default().reload(directory.path());
        assert_eq!(loaded.get(write.definition.id()), Some(&write.definition));
        write.rollback().unwrap();
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }

    #[test]
    fn test_theme_write_existing_rollback_restores_exact_bytes() {
        let directory = tempfile::tempdir().unwrap();
        let id = ThemeId::new("user:original");
        let source = include_str!("../../../assets/themes/ropy-light.toml");
        let path = directory.path().join("original.toml");
        std::fs::write(&path, source).unwrap();
        let mut changed = document();
        changed.name = "Changed".into();
        let write = ThemeWrite::save(directory.path(), &changed, Some((&id, source))).unwrap();
        assert_ne!(std::fs::read_to_string(&path).unwrap(), source);
        write.rollback().unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), source);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn test_theme_write_failed_restore_keeps_original_recovery_copy() {
        let directory = tempfile::tempdir().unwrap();
        let id = ThemeId::new("user:original");
        let source = include_str!("../../../assets/themes/ropy-light.toml");
        let path = directory.path().join("original.toml");
        std::fs::write(&path, source).unwrap();
        let write = ThemeWrite::save(directory.path(), &document(), Some((&id, source))).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        match write.rollback().unwrap_err() {
            ThemeWriteError::Restore { backup, .. } => {
                assert_eq!(std::fs::read_to_string(backup).unwrap(), source);
            }
            error => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn test_theme_write_external_change_is_not_overwritten() {
        let directory = tempfile::tempdir().unwrap();
        let id = ThemeId::new("user:original");
        let path = directory.path().join("original.toml");
        std::fs::write(&path, "external edit").unwrap();
        assert!(matches!(
            ThemeWrite::save(directory.path(), &document(), Some((&id, "old"))),
            Err(ThemeWriteError::Changed)
        ));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "external edit");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}

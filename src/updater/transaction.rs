//! Durable staging and a separate restart helper that can restore the old install.

use std::{
    fs::{self, File},
    io::Read as _,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

use super::{errors::UpdateError, installation::Installation};

mod parent;

const HELPER_ARG: &str = "--ropy-update-helper";
const CONFIRM_ARG: &str = "--ropy-update-confirm";

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Transaction {
    target: PathBuf,
    bundle: bool,
}

impl Transaction {
    fn root(&self) -> PathBuf {
        transaction_root(&self.target)
    }
    fn staged(&self) -> PathBuf {
        self.root().join("staged")
    }
    fn backup(&self) -> PathBuf {
        self.root().join("backup")
    }
    fn executable(&self) -> PathBuf {
        if self.bundle {
            self.target.join("Contents/MacOS/ropy")
        } else {
            self.target.clone()
        }
    }

    pub(crate) fn stage(target: &Path, source: &Path, bundle: bool) -> Result<Self, UpdateError> {
        let transaction = Self {
            target: target.to_path_buf(),
            bundle,
        };
        fs::create_dir_all(transaction.root())?;
        let _lock = transaction.lock()?;
        if transaction.backup().exists() {
            return Err(UpdateError::Replace(
                "a previous update requires recovery".into(),
            ));
        }
        let pending = tempfile::tempdir_in(transaction.root())?;
        let payload = pending.path().join("payload");
        copy_tree(source, &payload)?;
        remove_if_exists(&transaction.staged())?;
        fs::rename(payload, transaction.staged())?;
        let bytes =
            serde_json::to_vec(&transaction).map_err(|e| UpdateError::Parse(e.to_string()))?;
        let mut metadata = tempfile::NamedTempFile::new_in(transaction.root())?;
        std::io::Write::write_all(metadata.as_file_mut(), &bytes)?;
        metadata.as_file().sync_all()?;
        metadata
            .persist(transaction.root().join("transaction.json"))
            .map_err(|error| UpdateError::Io(error.error))?;
        Ok(transaction)
    }

    fn load(root: &Path) -> Result<Self, UpdateError> {
        let value: Self = serde_json::from_slice(&fs::read(root.join("transaction.json"))?)
            .map_err(|e| UpdateError::Parse(e.to_string()))?;
        if !value.target.is_absolute() || value.root() != root {
            return Err(UpdateError::Replace(
                "invalid update transaction location".into(),
            ));
        }
        Ok(value)
    }

    fn lock(&self) -> Result<File, UpdateError> {
        let lock = File::options()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root().join("lock"))?;
        lock.try_lock()
            .map_err(|e| UpdateError::Replace(e.to_string()))?;
        Ok(lock)
    }

    fn install(&self) -> Result<(), UpdateError> {
        if self.backup().exists() || !self.staged().exists() {
            return Err(UpdateError::Replace(
                "update is not ready for replacement".into(),
            ));
        }
        // Both renames stay on the installation filesystem. Never delete the old
        // installation until the replacement has acknowledged successful startup.
        fs::rename(&self.target, self.backup())?;
        if let Err(error) = fs::rename(self.staged(), &self.target) {
            fs::rename(self.backup(), &self.target)?;
            return Err(error.into());
        }
        Ok(())
    }

    fn rollback(&self) -> Result<(), UpdateError> {
        if self.backup().exists() {
            remove_if_exists(&self.target)?;
            fs::rename(self.backup(), &self.target)?;
        }
        Ok(())
    }
}

fn transaction_root(target: &Path) -> PathBuf {
    let name = target.file_name().unwrap_or_default().to_string_lossy();
    target.with_file_name(format!(".{name}-update"))
}

fn remove_if_exists(path: &Path) -> Result<(), UpdateError> {
    if path.is_dir() {
        fs::remove_dir_all(path)?;
    } else if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

fn copy_tree(source: &Path, target: &Path) -> Result<(), UpdateError> {
    let metadata = fs::symlink_metadata(source)?;
    // Update payloads deliberately contain no links: never copy outside the
    // validated archive tree or turn a bundle link into an arbitrary write.
    if metadata.file_type().is_symlink() {
        return Err(UpdateError::Extract(
            "symbolic links are not supported in update payloads".into(),
        ));
    }
    if metadata.is_dir() {
        fs::create_dir(target)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_tree(&entry.path(), &target.join(entry.file_name()))?;
        }
    } else {
        fs::copy(source, target)?;
        File::options().write(true).open(target)?.sync_all()?;
    }
    fs::set_permissions(target, metadata.permissions())?;
    Ok(())
}

pub(crate) fn is_staged() -> bool {
    Installation::current()
        .ok()
        .and_then(|install| install.target().ok().map(transaction_root))
        .is_some_and(|root| root.join("staged").exists() && root.join("transaction.json").exists())
}

pub(crate) fn start_restart() -> Result<Child, UpdateError> {
    let installation = Installation::current()?;
    start_helper(&transaction_root(installation.target()?))
}

fn start_helper(root: &Path) -> Result<Child, UpdateError> {
    let transaction = Transaction::load(root)?;
    let lock = transaction.lock()?;
    let helper = root.join(if cfg!(windows) {
        "helper.exe"
    } else {
        "helper"
    });
    fs::copy(std::env::current_exe()?, &helper)?;
    remove_if_exists(&root.join("ready"))?;
    drop(lock);
    let mut child = Command::new(helper)
        .arg(HELPER_ARG)
        .arg(root)
        .arg(std::process::id().to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(File::create(root.join("helper.log"))?)
        .spawn()
        .map_err(|e| UpdateError::Restart(e.to_string()))?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if child.try_wait()?.is_some() {
            return Err(UpdateError::Restart(
                "update helper exited before handoff".into(),
            ));
        }
        if fs::read_to_string(root.join("ready")).is_ok_and(|pid| pid == child.id().to_string()) {
            return Ok(child);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = child.kill();
    let _ = child.wait();
    Err(UpdateError::Restart(
        "update helper did not become ready".into(),
    ))
}

/// Run before initializing the GUI or acquiring the Windows single-instance lock.
pub(crate) fn handle_startup() -> Result<bool, UpdateError> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == HELPER_ARG) {
        let root = args
            .get(1)
            .ok_or_else(|| UpdateError::Restart("missing transaction path".into()))?;
        let pid = args
            .get(2)
            .and_then(|value| value.to_str())
            .and_then(|value| value.parse().ok())
            .ok_or_else(|| UpdateError::Restart("missing parent PID".into()))?;
        run_helper(Path::new(root), pid)?;
        return Ok(true);
    }
    if args.first().is_some_and(|arg| arg == CONFIRM_ARG) {
        return Ok(false);
    }
    let installation = Installation::current()?;
    if let Ok(target) = installation.target() {
        let root = transaction_root(target);
        if root.join("backup").exists() {
            let transaction = Transaction::load(&root)?;
            // A live helper already owns recovery. An interrupted transaction
            // is resumed by a helper outside the installation being replaced.
            if let Ok(lock) = transaction.lock() {
                if root.join("healthy").exists() {
                    // A failed backup cleanup must not roll back an acknowledged
                    // healthy version on the next launch.
                    if let Err(error) = remove_if_exists(&transaction.backup()) {
                        tracing::warn!(%error, "could not remove acknowledged update backup");
                    }
                    return Ok(false);
                }
                drop(lock);
                let _child = start_helper(&root)?;
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn run_helper(root: &Path, pid: u32) -> Result<(), UpdateError> {
    let transaction = Transaction::load(root)?;
    let _lock = transaction.lock()?;
    let parent = parent::ParentProcess::open(pid)?;
    fs::write(root.join("ready"), std::process::id().to_string())?;
    // The parent retains the pipe until it exits. This avoids launching the new
    // process while the old Windows instance still owns its mutex.
    let mut input = Vec::new();
    std::io::stdin().read_to_end(&mut input)?;
    parent.wait()?;
    if transaction.backup().exists() {
        return restore_and_launch(&transaction);
    }
    remove_if_exists(&root.join("healthy"))?;
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match transaction.install() {
            Ok(()) => break,
            Err(error) if Instant::now() >= deadline || transaction.backup().exists() => {
                tracing::error!(%error, "could not install staged update");
                return restore_and_launch(&transaction);
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let launch = Command::new(transaction.executable())
        .arg(CONFIRM_ARG)
        .arg(root)
        .spawn();
    if let Ok(mut child) = launch {
        let deadline = Instant::now() + Duration::from_secs(45);
        while Instant::now() < deadline {
            if child.try_wait()?.is_some() {
                break;
            }
            if root.join("healthy").exists() {
                remove_if_exists(&transaction.backup())?;
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    restore_and_launch(&transaction)
}

fn restore_and_launch(transaction: &Transaction) -> Result<(), UpdateError> {
    transaction.rollback()?;
    remove_if_exists(&transaction.staged())?;
    fs::write(transaction.root().join("rolled-back"), b"update failed")?;
    Command::new(transaction.executable()).spawn()?;
    Ok(())
}

pub(crate) fn take_rollback_notice() -> bool {
    Installation::current()
        .ok()
        .and_then(|installation| installation.target().ok().map(transaction_root))
        .is_some_and(|root| fs::remove_file(root.join("rolled-back")).is_ok())
}

pub(crate) fn confirm_startup() -> Result<(), UpdateError> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == CONFIRM_ARG) {
        let root = args
            .get(1)
            .ok_or_else(|| UpdateError::Restart("missing confirmation path".into()))?;
        let root = Path::new(root);
        let transaction = Transaction::load(root)?;
        if fs::canonicalize(transaction.executable())? != std::env::current_exe()? {
            return Err(UpdateError::Restart(
                "confirmation executable mismatch".into(),
            ));
        }
        fs::write(root.join("healthy"), b"healthy")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_transaction_failed_staging_preserves_previous_complete_payload() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("ropy");
        let source = dir.path().join("new");
        fs::write(&target, "old").unwrap();
        fs::write(&source, "complete").unwrap();
        let transaction = Transaction::stage(&target, &source, false).unwrap();
        assert!(Transaction::stage(&target, &dir.path().join("missing"), false).is_err());
        assert_eq!(
            fs::read_to_string(transaction.staged()).unwrap(),
            "complete"
        );
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_transaction_missing_staged_payload_keeps_original_installation() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("ropy");
        fs::write(&target, "old").unwrap();
        let transaction = Transaction {
            target: target.clone(),
            bundle: false,
        };
        fs::create_dir_all(transaction.root()).unwrap();
        assert!(transaction.install().is_err());
        assert_eq!(fs::read_to_string(&target).unwrap(), "old");
        assert!(!transaction.backup().exists());
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_transaction_install_and_rollback_restore_original() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("ropy");
        let new = dir.path().join("new");
        fs::write(&target, "old").unwrap();
        fs::write(&new, "new").unwrap();
        let transaction = Transaction::stage(&target, &new, false).unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "old");
        transaction.install().unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        transaction.rollback().unwrap();
        assert_eq!(fs::read_to_string(&target).unwrap(), "old");
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_transaction_bundle_replaces_resources_and_restores_them() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("Ropy.app");
        let new = dir.path().join("new.app");
        for (path, text) in [(&target, "old"), (&new, "new")] {
            fs::create_dir_all(path.join("Contents/MacOS")).unwrap();
            fs::write(path.join("Contents/MacOS/ropy"), text).unwrap();
            fs::write(path.join("Contents/Info.plist"), text).unwrap();
        }
        let transaction = Transaction::stage(&target, &new, true).unwrap();
        transaction.install().unwrap();
        assert_eq!(
            fs::read_to_string(target.join("Contents/Info.plist")).unwrap(),
            "new"
        );
        transaction.rollback().unwrap();
        assert_eq!(
            fs::read_to_string(target.join("Contents/Info.plist")).unwrap(),
            "old"
        );
    }
}

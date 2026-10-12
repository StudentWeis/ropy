//! Linux session detection and single-instance activation from compositor shortcuts.

mod scaling;

use std::{
    ffi::OsStr,
    fs::{DirBuilder, File, OpenOptions, TryLockError},
    io,
    os::unix::{
        fs::{DirBuilderExt, MetadataExt, OpenOptionsExt},
        net::UnixDatagram,
    },
    path::{Path, PathBuf},
    time::Duration,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum StartupError {
    #[error("Linux popup windows currently require X11 or XWayland (DISPLAY is missing)")]
    MissingDisplay,
    #[error("activation directory must be owned by the current user with mode 0700: {0}")]
    UnsafeActivationDirectory(PathBuf),
    #[error("failed to initialize Linux activation: {0}")]
    Activation(#[from] io::Error),
}

/// The lock lives for the whole GUI session, including while the popup is hidden.
/// Socket cleanup happens only after acquiring the lock, so an old socket from a
/// crashed process can never cause us to unlink another running instance's socket.
#[derive(Debug)]
pub(crate) struct Instance {
    _lock: File,
    socket: UnixDatagram,
    show_on_start: bool,
}

impl gpui_kit::Global for Instance {}

pub(crate) fn configure_xwayland_scale() -> io::Result<()> {
    scaling::configure_xwayland_scale()
}

pub(crate) fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty())
}

impl Instance {
    pub(crate) fn start(show_on_start: bool) -> Result<Option<Self>, StartupError> {
        if std::env::var_os("DISPLAY").is_none_or(|display| display.is_empty()) {
            return Err(StartupError::MissingDisplay);
        }
        let runtime = std::env::var_os("XDG_RUNTIME_DIR");
        let display = std::env::var_os("WAYLAND_DISPLAY")
            .filter(|display| !display.is_empty())
            .or_else(|| std::env::var_os("DISPLAY"))
            .unwrap_or_default();
        let directory = activation_directory(
            runtime.as_deref().map(Path::new),
            &std::env::temp_dir(),
            &display,
        )?;
        let mut instance = Self::acquire(&directory)?;
        if let Some(instance) = &mut instance {
            instance.show_on_start = show_on_start;
        }
        Ok(instance)
    }

    fn acquire(directory: &Path) -> io::Result<Option<Self>> {
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(directory.join("instance.lock"))?;
        let socket_path = directory.join("activation.sock");
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                forward_activation(&socket_path)?;
                return Ok(None);
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
        match std::fs::remove_file(&socket_path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let socket = UnixDatagram::bind(socket_path)?;
        Ok(Some(Self {
            _lock: lock,
            socket,
            show_on_start: false,
        }))
    }

    pub(crate) fn install(
        self,
        window: gpui_kit::WindowHandle<gpui_kit::component::Root>,
        cx: &mut gpui_kit::App,
    ) {
        let socket = match self.socket.try_clone() {
            Ok(socket) => socket,
            Err(error) => {
                tracing::error!(%error, "failed to start activation listener");
                cx.set_global(self);
                return;
            }
        };
        let (tx, rx) = async_channel::bounded(1);
        if self.show_on_start {
            let _ = tx.try_send(());
        }
        let result = std::thread::Builder::new()
            .name("linux-activation".into())
            .spawn(move || {
                let mut message = [0; 16];
                while let Ok(size) = socket.recv(&mut message) {
                    if &message[..size] == b"show" {
                        match tx.try_send(()) {
                            Ok(()) | Err(async_channel::TrySendError::Full(())) => {}
                            Err(async_channel::TrySendError::Closed(())) => break,
                        }
                    }
                }
            });
        if let Err(error) = result {
            tracing::error!(%error, "failed to start activation listener");
        }
        cx.set_global(self);
        cx.spawn(async move |cx| {
            while rx.recv().await.is_ok() {
                let _ = window.update(cx, |_, window, cx| {
                    window.dispatch_action(Box::new(crate::gui::board::Active), cx);
                });
            }
        })
        .detach();
    }
}

fn activation_directory(
    runtime: Option<&Path>,
    temporary: &Path,
    display: &OsStr,
) -> Result<PathBuf, StartupError> {
    // Preserve existing runtime endpoints, including those of a running instance.
    // Hash display names so they cannot introduce path components.
    let session = seahash::hash(display.as_encoded_bytes());
    if let Some(runtime) = runtime.filter(|path| !path.as_os_str().is_empty()) {
        let directory = runtime.join(format!("ropy-{session:016x}"));
        std::fs::create_dir_all(&directory)?;
        return Ok(directory);
    }

    // X11 does not require XDG_RUNTIME_DIR. A deterministic private directory
    // in the temporary root keeps activation and stale-socket recovery available.
    // SAFETY: geteuid has no preconditions and only reads the effective user ID.
    let uid = unsafe { libc::geteuid() };
    let directory = temporary.join(format!("ropy-{uid}-{session:016x}"));
    match DirBuilder::new().mode(0o700).create(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    // Do not follow symlinks or repair an untrusted pre-existing path: no lock
    // or socket inside it may be opened or removed before this check succeeds.
    let metadata = std::fs::symlink_metadata(&directory)?;
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o777 != 0o700 {
        return Err(StartupError::UnsafeActivationDirectory(directory));
    }
    Ok(directory)
}

fn forward_activation(path: &Path) -> io::Result<()> {
    let socket = UnixDatagram::unbound()?;
    socket.set_write_timeout(Some(Duration::from_secs(1)))?;
    // A concurrent first launch may hold the lock before binding its socket.
    for attempt in 0..20 {
        match socket.send_to(b"show", path) {
            Ok(_) => return Ok(()),
            Err(error)
                if attempt < 19
                    && matches!(
                        error.kind(),
                        io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
                    ) =>
            {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("the final send attempt returns its result")
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case(None)]
    #[case(Some(Path::new("")))]
    fn test_activation_directory_missing_runtime_supports_single_instance(
        #[case] runtime: Option<&Path>,
    ) {
        let temporary = tempfile::tempdir().unwrap();
        let directory = activation_directory(runtime, temporary.path(), OsStr::new(":0")).unwrap();
        assert!(directory.starts_with(temporary.path()));
        let metadata = std::fs::symlink_metadata(&directory).unwrap();
        assert_eq!(metadata.mode() & 0o777, 0o700);
        assert_eq!(metadata.uid(), temporary.path().metadata().unwrap().uid());
        let first = Instance::acquire(&directory).unwrap().unwrap();
        first
            .socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let same = activation_directory(runtime, temporary.path(), OsStr::new(":0")).unwrap();
        assert!(Instance::acquire(&same).unwrap().is_none());
        let mut message = [0; 16];
        let size = first.socket.recv(&mut message).unwrap();
        assert_eq!(&message[..size], b"show");
        let other = activation_directory(runtime, temporary.path(), OsStr::new(":1")).unwrap();
        assert!(Instance::acquire(&other).unwrap().is_some());
        drop(first);
        assert!(Instance::acquire(&directory).unwrap().is_some());
    }

    #[test]
    fn test_activation_directory_configured_runtime_preserves_endpoint() {
        let runtime = tempfile::tempdir().unwrap();
        let temporary = tempfile::tempdir().unwrap();
        let display = OsStr::new("wayland-1");
        let directory =
            activation_directory(Some(runtime.path()), temporary.path(), display).unwrap();
        assert_eq!(
            directory,
            runtime.path().join(format!(
                "ropy-{:016x}",
                seahash::hash(display.as_encoded_bytes())
            ))
        );
    }

    #[rstest]
    #[case(false)]
    #[case(true)]
    fn test_activation_directory_unsafe_existing_path_rejected(#[case] link: bool) {
        let temporary = tempfile::tempdir().unwrap();
        let display = OsStr::new(":0");
        let directory = activation_directory(None, temporary.path(), display).unwrap();
        if link {
            std::fs::remove_dir(&directory).unwrap();
            symlink(temporary.path(), &directory).unwrap();
        } else {
            std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o777)).unwrap();
        }
        assert!(activation_directory(None, temporary.path(), display).is_err());
    }

    #[test]
    fn test_instance_second_launch_forwards_show_to_first() {
        let directory = tempfile::tempdir().unwrap();
        let first = Instance::acquire(directory.path()).unwrap().unwrap();
        first
            .socket
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        assert!(Instance::acquire(directory.path()).unwrap().is_none());
        let mut message = [0; 16];
        let size = first.socket.recv(&mut message).unwrap();
        assert_eq!(&message[..size], b"show");
    }

    #[test]
    fn test_instance_after_exit_reclaims_stale_socket() {
        let directory = tempfile::tempdir().unwrap();
        drop(Instance::acquire(directory.path()).unwrap().unwrap());
        assert!(Instance::acquire(directory.path()).unwrap().is_some());
    }

    #[test]
    fn test_instance_independent_sessions_do_not_forward() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        let _first = Instance::acquire(first.path()).unwrap().unwrap();
        assert!(Instance::acquire(second.path()).unwrap().is_some());
    }
}

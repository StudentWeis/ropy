//! Linux session detection and single-instance activation from compositor shortcuts.

use std::{
    fs::{File, OpenOptions, TryLockError},
    io,
    os::unix::{fs::OpenOptionsExt, net::UnixDatagram},
    path::{Path, PathBuf},
    time::Duration,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum StartupError {
    #[error("Linux popup windows currently require X11 or XWayland (DISPLAY is missing)")]
    MissingDisplay,
    #[error("XDG_RUNTIME_DIR is missing")]
    MissingRuntimeDirectory,
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

pub(crate) fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some_and(|display| !display.is_empty())
}

impl Instance {
    pub(crate) fn start(show_on_start: bool) -> Result<Option<Self>, StartupError> {
        if std::env::var_os("DISPLAY").is_none_or(|display| display.is_empty()) {
            return Err(StartupError::MissingDisplay);
        }
        let runtime =
            std::env::var_os("XDG_RUNTIME_DIR").ok_or(StartupError::MissingRuntimeDirectory)?;
        let display = std::env::var_os("WAYLAND_DISPLAY")
            .filter(|display| !display.is_empty())
            .or_else(|| std::env::var_os("DISPLAY"))
            .unwrap_or_default();
        // Separate compositor sessions belonging to the same user. Hash the
        // display name rather than allowing it to become a filesystem path.
        let session = seahash::hash(display.as_encoded_bytes());
        let directory = PathBuf::from(runtime).join(format!("ropy-{session:016x}"));
        std::fs::create_dir_all(&directory)?;
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
    use super::*;

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

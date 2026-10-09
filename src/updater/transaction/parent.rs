//! Keep the restart helper behind the old process's actual exit, not a delay.

use super::UpdateError;

#[cfg(unix)]
pub(super) struct ParentProcess {
    pid: libc::pid_t,
}

#[cfg(unix)]
impl ParentProcess {
    pub(super) fn open(expected_pid: u32) -> Result<Self, UpdateError> {
        // SAFETY: getppid has no arguments or memory preconditions.
        let pid = unsafe { libc::getppid() };
        if pid <= 1 || u32::try_from(pid).ok() != Some(expected_pid) {
            return Err(UpdateError::Restart("update parent already exited".into()));
        }
        Ok(Self { pid })
    }

    pub(super) fn wait(&self) -> Result<(), UpdateError> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        // SAFETY: getppid only reads process identity. Reparenting proves that
        // the original parent exited and avoids PID reuse races with kill(0).
        while unsafe { libc::getppid() } == self.pid {
            if std::time::Instant::now() >= deadline {
                return Err(UpdateError::Restart("parent did not exit".into()));
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        Ok(())
    }
}

#[cfg(windows)]
pub(super) struct ParentProcess(std::os::windows::io::OwnedHandle);

#[cfg(windows)]
impl ParentProcess {
    pub(super) fn open(pid: u32) -> Result<Self, UpdateError> {
        use std::os::windows::io::FromRawHandle as _;

        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_SYNCHRONIZE};
        // SAFETY: request only synchronization rights for the supplied parent
        // PID. A non-null handle is immediately owned and closed by OwnedHandle.
        let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        if handle.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }
        // SAFETY: OpenProcess returned a unique owned process handle.
        Ok(Self(unsafe {
            std::os::windows::io::OwnedHandle::from_raw_handle(handle)
        }))
    }

    pub(super) fn wait(&self) -> Result<(), UpdateError> {
        use std::os::windows::io::AsRawHandle as _;

        use windows_sys::Win32::{
            Foundation::WAIT_OBJECT_0, System::Threading::WaitForSingleObject,
        };
        // SAFETY: this struct keeps the process handle alive for the wait.
        if unsafe { WaitForSingleObject(self.0.as_raw_handle(), 60_000) } == WAIT_OBJECT_0 {
            Ok(())
        } else {
            Err(UpdateError::Restart("parent did not exit".into()))
        }
    }
}

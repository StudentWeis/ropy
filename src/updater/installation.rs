//! Installation ownership and update artifact selection.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Installation {
    Standalone(PathBuf),
    Bundle(PathBuf),
    Managed,
}

impl Installation {
    pub(crate) fn current() -> Result<Self, super::errors::UpdateError> {
        Ok(Self::detect(&std::env::current_exe()?))
    }

    pub(crate) fn detect(exe: &Path) -> Self {
        if exe
            .parent()
            .is_some_and(|parent| parent.join(".ropy-managed").exists())
        {
            return Self::Managed;
        }
        let text = exe
            .to_string_lossy()
            .replace('\\', "/")
            .to_ascii_lowercase();
        if text.contains("/scoop/apps/")
            || text.contains("/cellar/")
            || text.contains("/caskroom/")
            || text.contains("/nix/store/")
            || text.starts_with("/usr/bin/")
            || text.starts_with("/usr/lib/")
            || text.starts_with("/snap/")
            || text.starts_with("/app/")
        {
            return Self::Managed;
        }
        if let Some(bundle) = exe
            .ancestors()
            .find(|path| path.extension().is_some_and(|ext| ext == "app"))
        {
            // Package publishers can opt out when their bundle lives outside
            // the package manager's own installation prefix.
            if bundle.join("Contents/.ropy-managed").exists() {
                return Self::Managed;
            }
            return Self::Bundle(bundle.to_path_buf());
        }
        Self::Standalone(exe.to_path_buf())
    }

    pub(crate) const fn is_bundle(&self) -> bool {
        matches!(self, Self::Bundle(_))
    }

    pub(crate) fn target(&self) -> Result<&Path, super::errors::UpdateError> {
        match self {
            Self::Standalone(path) | Self::Bundle(path) => Ok(path),
            Self::Managed => Err(super::errors::UpdateError::ManagedInstallation),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[rstest::rstest]
    #[case("C:\\Users\\me\\scoop\\apps\\ropy\\0.5.6\\ropy.exe")]
    #[case("/opt/homebrew/Cellar/ropy/0.5.6/bin/ropy")]
    #[case("/usr/bin/ropy")]
    fn test_installation_managed_path_rejects_replacement(#[case] path: &str) {
        assert!(Installation::detect(Path::new(path)).target().is_err());
    }
    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_installation_external_marker_blocks_custom_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("ropy");
        std::fs::write(dir.path().join(".ropy-managed"), b"").unwrap();
        assert_eq!(Installation::detect(&executable), Installation::Managed);
    }

    #[test]
    fn test_installation_bundle_targets_whole_app() {
        assert_eq!(
            Installation::detect(Path::new("/Applications/Ropy.app/Contents/MacOS/ropy")),
            Installation::Bundle(PathBuf::from("/Applications/Ropy.app"))
        );
    }
}

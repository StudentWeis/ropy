//! XDG autostart files owned by Ropy, with desktop-entry command encoding.

use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use super::AutoStartError;

pub(super) struct DesktopEntry {
    path: PathBuf,
    contents: String,
}

impl DesktopEntry {
    pub(super) fn new(config_dir: &Path, app_name: &str, app_path: &str) -> Self {
        Self {
            path: config_dir
                .join("autostart")
                .join(format!("{app_name}.desktop")),
            contents: format!(
                "[Desktop Entry]\nType=Application\nVersion=1.0\nName={app_name}\nExec={}\nStartupNotify=false\nTerminal=false\n",
                quote_exec_path(app_path),
            ),
        }
    }

    pub(super) fn sync_state(&self, enabled: bool) -> Result<(), AutoStartError> {
        if !enabled {
            return match fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(AutoStartError::Disable(error.to_string())),
            };
        }
        match fs::read_to_string(&self.path) {
            Ok(current) if current == self.contents => return Ok(()),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(AutoStartError::StatusCheck(error.to_string())),
        }
        self.write()
            .map_err(|error| AutoStartError::Enable(error.to_string()))
    }

    fn write(&self) -> std::io::Result<()> {
        // The path always has the autostart directory supplied by new().
        let directory = self
            .path
            .parent()
            .ok_or_else(|| std::io::Error::other("missing autostart directory"))?;
        fs::create_dir_all(directory)?;
        let mut pending = tempfile::NamedTempFile::new_in(directory)?;
        pending.write_all(self.contents.as_bytes())?;
        pending.as_file().sync_all()?;
        pending.persist(&self.path).map_err(|error| error.error)?;
        Ok(())
    }
}

fn quote_exec_path(path: &str) -> String {
    // Desktop Entry string unescaping precedes Exec argument parsing. Encode
    // both layers, including literal field-code markers; this is not shell input.
    let mut encoded = String::from("\"");
    for character in path.chars() {
        match character {
            '\\' => encoded.push_str(r"\\\\"),
            '"' | '`' | '$' => {
                encoded.push_str(r"\\");
                encoded.push(character);
            }
            '\n' => encoded.push_str(r"\n"),
            '\t' => encoded.push_str(r"\t"),
            '\r' => encoded.push_str(r"\r"),
            '%' => encoded.push_str("%%"),
            _ => encoded.push(character),
        }
    }
    encoded.push('"');
    encoded
}

#[cfg(test)]
mod tests {
    use super::{DesktopEntry, quote_exec_path};

    #[test]
    fn test_sync_uses_selected_config_directory_and_preserves_other_profile() {
        let directory = tempfile::tempdir().unwrap();
        let legacy = directory.path().join("default/autostart/Ropy.desktop");
        std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        std::fs::write(&legacy, "another profile").unwrap();
        let config = directory.path().join("custom config");
        let entry = DesktopEntry::new(&config, "Ropy", "/opt/ropy");

        entry.sync_state(true).unwrap();
        let path = config.join("autostart/Ropy.desktop");
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("Exec=\"/opt/ropy\"\n")
        );
        assert_eq!(std::fs::read_to_string(&legacy).unwrap(), "another profile");
        entry.sync_state(false).unwrap();
        entry.sync_state(false).unwrap();
        assert!(!path.exists());
        assert_eq!(std::fs::read_to_string(&legacy).unwrap(), "another profile");
    }

    #[test]
    fn test_sync_repairs_enabled_entry_after_executable_moves() {
        let directory = tempfile::tempdir().unwrap();
        DesktopEntry::new(directory.path(), "Ropy", "/opt/old/ropy")
            .sync_state(true)
            .unwrap();
        let entry = DesktopEntry::new(directory.path(), "Ropy", "/opt/My Apps/ropy");
        entry.sync_state(true).unwrap();
        entry.sync_state(true).unwrap();
        let contents =
            std::fs::read_to_string(directory.path().join("autostart/Ropy.desktop")).unwrap();
        assert!(contents.contains("Exec=\"/opt/My Apps/ropy\"\n"));
        assert!(!contents.contains("/opt/old/ropy"));
    }

    #[test]
    fn test_sync_reports_unwritable_destination_without_replacing_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("autostart/Ropy.desktop");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("sentinel"), "keep").unwrap();
        let entry = DesktopEntry::new(directory.path(), "Ropy", "/opt/ropy");
        assert!(entry.sync_state(true).is_err());
        assert_eq!(
            std::fs::read_to_string(path.join("sentinel")).unwrap(),
            "keep"
        );
    }

    #[rstest::rstest]
    #[case("/opt/My Apps/ropy", "\"/opt/My Apps/ropy\"")]
    #[case("/opt/%f/ropy", "\"/opt/%%f/ropy\"")]
    #[case("/opt/雪/ropy", "\"/opt/雪/ropy\"")]
    #[case(
        "/opt/line\nbreak\tand\rreturn/ropy",
        "\"/opt/line\\nbreak\\tand\\rreturn/ropy\""
    )]
    #[case(r#"/opt/a"b\c$d`e/ropy"#, r#""/opt/a\\"b\\\\c\\$d\\`e/ropy""#)]
    fn test_desktop_exec_preserves_literal_path(#[case] path: &str, #[case] expected: &str) {
        assert_eq!(quote_exec_path(path), expected);
    }
}

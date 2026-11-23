use std::{env, fs, io, path::Path};

pub(super) fn is_running_executable(expected: &Path) -> io::Result<bool> {
    Ok(fs::canonicalize(expected)? == fs::canonicalize(env::current_exe()?)?)
}

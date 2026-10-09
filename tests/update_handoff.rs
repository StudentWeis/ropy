//! Exercise the real updater helper across process exit and startup failure.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

struct ParentGuard(Child);

impl Drop for ParentGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !condition() {
        assert!(Instant::now() < deadline, "updater subprocess timed out");
        thread::sleep(Duration::from_millis(20));
    }
}

#[expect(clippy::expect_used)]
fn fixture(directory: &Path) -> PathBuf {
    let binary = directory.join(if cfg!(windows) {
        "fixture.exe"
    } else {
        "fixture"
    });
    let status = Command::new("rustc")
        .arg("--edition=2024")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/update_process.rs"))
        .arg("-o")
        .arg(&binary)
        .status()
        .expect("compile subprocess fixture");
    assert!(status.success());
    binary
}

#[rstest::rstest]
#[case(false)]
#[case(true)]
fn test_restart_handoff_waits_for_parent_and_recovers_failed_startup(#[case] fail_startup: bool) {
    run_handoff(fail_startup);
}

#[expect(clippy::expect_used)]
fn run_handoff(fail_startup: bool) {
    let temporary = tempfile::tempdir().expect("temporary installation");
    let directory = temporary
        .path()
        .canonicalize()
        .expect("absolute installation path");
    let directory = directory.as_path();
    let binary = fixture(directory);
    let target = directory.join(if cfg!(windows) { "ropy.exe" } else { "ropy" });
    let root = target.with_file_name(format!(
        ".{}-update",
        target.file_name().expect("filename").to_string_lossy()
    ));
    fs::create_dir(&root).expect("transaction directory");
    fs::copy(&binary, &target).expect("original executable");
    fs::copy(&binary, root.join("staged")).expect("staged executable");
    fs::write(
        root.join("transaction.json"),
        serde_json::to_vec(&serde_json::json!({"target": target, "bundle": false}))
            .expect("serialize transaction"),
    )
    .expect("persist transaction");
    if fail_startup {
        fs::write(root.join("fail-startup"), "fail").expect("failure scenario");
    }
    let mut parent = ParentGuard(
        Command::new(binary)
            .arg("parent")
            .arg(env!("CARGO_BIN_EXE_ropy"))
            .arg(&root)
            .spawn()
            .expect("start parent fixture"),
    );
    wait_until(|| root.join("parent-ready").exists());
    assert!(!root.join("new-started").exists());
    assert!(!root.join("backup").exists());
    assert!(root.join("staged").exists());
    fs::write(root.join("exit-parent"), "exit").expect("request parent exit");
    assert!(parent.0.wait().expect("wait for parent exit").success());
    if fail_startup {
        wait_until(|| root.join("old-started").exists());
        assert!(root.join("rolled-back").exists());
    } else {
        wait_until(|| root.join("healthy").exists() && !root.join("backup").exists());
        assert!(!root.join("old-started").exists());
    }
    assert!(root.join("new-started").exists());
    assert!(!root.join("staged").exists());
    assert!(target.is_file());
    // On Windows an executing image cannot be removed until its process exits.
    wait_until(|| fs::remove_file(&target).is_ok());
}

//! Exercise the real updater helper across process exit and startup failure.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

use sha2::{Digest as _, Sha256};

struct ParentGuard(Child);

impl Drop for ParentGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_until(stage: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !condition() {
        assert!(
            Instant::now() < deadline,
            "updater subprocess timed out: {stage}"
        );
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
    let mut executable_sha256 = String::new();
    for byte in Sha256::digest(fs::read(&target).expect("read fixture")) {
        use std::fmt::Write as _;
        write!(executable_sha256, "{byte:02x}").expect("format digest");
    }
    fs::write(
        root.join("transaction.json"),
        serde_json::to_vec(&serde_json::json!({
            "target": target,
            "bundle": false,
            "origin": {
                "installed_version": env!("CARGO_PKG_VERSION"),
                "release_version": "999.0.0",
                "executable_sha256": executable_sha256,
            },
        }))
        .expect("serialize transaction"),
    )
    .expect("persist transaction");
    if fail_startup {
        fs::write(root.join("fail-startup"), "fail").expect("failure scenario");
    }
    let process_log = fs::File::create(root.join("process.log")).expect("process log");
    let mut parent = ParentGuard(
        Command::new(binary)
            .arg("parent")
            .arg(env!("CARGO_BIN_EXE_ropy"))
            .arg(&root)
            .stdout(process_log.try_clone().expect("clone process log"))
            .stderr(process_log)
            .spawn()
            .expect("start parent fixture"),
    );
    wait_until("parent ready", || {
        let status = parent.0.try_wait().expect("parent status");
        assert!(
            status.is_none(),
            "parent exited before ready: {status:?}; log: {}",
            fs::read_to_string(root.join("process.log")).unwrap_or_default()
        );
        root.join("parent-ready").exists()
    });
    assert!(!root.join("new-started").exists());
    assert!(!root.join("backup").exists());
    assert!(root.join("staged").exists());
    fs::write(root.join("exit-parent"), "exit").expect("request parent exit");
    assert!(parent.0.wait().expect("wait for parent exit").success());
    if fail_startup {
        wait_until("rollback startup", || root.join("old-started").exists());
        assert!(root.join("rolled-back").exists());
    } else {
        wait_until("healthy startup and backup cleanup", || {
            assert!(
                !root.join("rolled-back").exists(),
                "healthy fixture rolled back; new_started={}; log: {}",
                root.join("new-started").exists(),
                fs::read_to_string(root.join("process.log")).unwrap_or_default()
            );
            root.join("healthy").exists() && !root.join("backup").exists()
        });
        assert!(!root.join("old-started").exists());
    }
    assert!(root.join("new-started").exists());
    assert!(!root.join("staged").exists());
    assert!(target.is_file());
    // On Windows an executing image cannot be removed until its process exits.
    wait_until("updated executable release", || {
        fs::remove_file(&target).is_ok()
    });
}

//! Subprocess fixture for updater handoff tests. Compiled for the host by rustc.

use std::{env, fs, path::Path, process::{Command, Stdio}, thread, time::{Duration, Instant}};

fn main() {
    let args: Vec<_> = env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "parent") {
        let root = Path::new(&args[2]);
        let mut child = Command::new(&args[1])
            .arg("--ropy-update-helper").arg(root).arg(std::process::id().to_string())
            .stdin(Stdio::piped()).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while Instant::now() < deadline {
            assert!(child.try_wait().unwrap().is_none(), "helper exited before ready");
            if fs::read_to_string(root.join("ready")).is_ok_and(|pid| pid == child.id().to_string()) {
                fs::write(root.join("parent-ready"), "ready").unwrap();
                while !root.join("exit-parent").exists() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(20));
                }
                // Keep the pipe alive until exit; the helper must wait for this process.
                std::process::exit(0);
            }
            thread::sleep(Duration::from_millis(20));
        }
        child.kill().unwrap();
        child.wait().unwrap();
        panic!("helper never became ready");
    }
    if args.first().is_some_and(|arg| arg == "--ropy-update-confirm") {
        let root = Path::new(&args[1]);
        fs::write(root.join("new-started"), "started").unwrap();
        if root.join("fail-startup").exists() { std::process::exit(1); }
        fs::write(root.join("healthy"), "healthy").unwrap();
        thread::sleep(Duration::from_secs(1));
        return;
    }
    let exe = env::current_exe().unwrap();
    let root = exe.with_file_name(format!(".{}-update", exe.file_name().unwrap().to_string_lossy()));
    fs::write(root.join("old-started"), "restored").unwrap();
}

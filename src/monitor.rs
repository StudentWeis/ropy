#[cfg(debug_assertions)]
use std::{thread, time::Duration};
#[cfg(debug_assertions)]
use sysinfo::{Pid, ProcessesToUpdate, System};

/// Spawn a background thread that periodically prints the OS RSS for this process
///
/// interval: how frequently to print (Duration)
#[cfg(debug_assertions)]
pub fn spawn_rss_monitor(interval: Duration) -> std::thread::JoinHandle<()> {
    let pid = Pid::from(std::process::id() as usize);
    thread::spawn(move || {
        let mut sys = System::new();
        loop {
            // Refresh only the process to reduce overhead
            sys.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
            if let Some(proc) = sys.process(pid) {
                // Normalize units from sysinfo depending on platform and print MB only
                let rss_raw = proc.memory();
                #[cfg(target_os = "macos")]
                let rss_bytes = rss_raw;
                #[cfg(not(target_os = "macos"))]
                let rss_bytes = (rss_raw as u64) * 1024;
                let rss_mb = rss_bytes as f64 / 1024.0 / 1024.0;
                println!("[rss-monitor] memory={:.2} MB", rss_mb);
            }
            thread::sleep(interval);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn smoke() {
        // Quick smoke test: spawn monitor and sleep a short while
        let _handle = spawn_rss_monitor(Duration::from_millis(200));
        std::thread::sleep(Duration::from_millis(600));
        // The monitor thread is long-lived and detached; we just let it drop
    }
}

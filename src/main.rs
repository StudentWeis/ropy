mod clipboard;
mod gui;
mod monitor;

fn main() {
    // 监控程序内存使用情况，仅在调试模式下启用
    #[cfg(debug_assertions)]
    let _monitor_handle = monitor::spawn_rss_monitor(std::time::Duration::from_secs(2));

    gui::launch_app();
}

mod gui;
mod monitor;
mod clipboard;

fn main() {
    #[cfg(debug_assertions)]
    let _monitor_handle = monitor::spawn_rss_monitor(std::time::Duration::from_secs(2));

    gui::launch_app();
}

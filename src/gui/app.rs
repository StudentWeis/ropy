use crate::clipboard;
use crate::gui::board::RopyBoard;
use crate::repository::{ClipboardRecord, ClipboardRepository};
use gpui::{
    App, AppContext, Application, AsyncApp, Bounds, WindowBounds, WindowHandle, WindowKind,
    WindowOptions, px, size,
};
use std::sync::atomic::AtomicBool;
use std::sync::{
    Arc, Mutex,
    mpsc::{self, channel},
};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "macos")]
use objc2::{class, msg_send, runtime::AnyObject};

#[cfg(target_os = "macos")]
fn set_activation_policy_accessory() {
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        // NSApplicationActivationPolicyAccessory = 1
        // 将应用设置为 Accessory 模式，这样它就不会出现在 Dock 和 Cmd+Tab 切换器中
        let _succeeded: bool = msg_send![app, setActivationPolicy: 1isize];
    }
}

fn initialize_repository() -> Option<Arc<ClipboardRepository>> {
    match ClipboardRepository::new() {
        Ok(repo) => {
            println!("[ropy] 剪切板历史记录仓库初始化成功");
            Some(Arc::new(repo))
        }
        Err(e) => {
            eprintln!("[ropy] 剪切板历史记录仓库初始化失败: {}", e);
            None
        }
    }
}

fn load_initial_records(repository: &Option<Arc<ClipboardRepository>>) -> Vec<ClipboardRecord> {
    repository
        .as_ref()
        .and_then(|repo| repo.get_recent(50).ok())
        .unwrap_or_default()
}

fn start_clipboard_monitor() -> (mpsc::Receiver<String>, thread::JoinHandle<()>) {
    let (clipboard_tx, clipboard_rx) = channel::<String>();
    let listener_handle =
        clipboard::start_clipboard_monitor(clipboard_tx, Duration::from_millis(300));
    (clipboard_rx, listener_handle)
}

fn start_hotkey_monitor() -> (mpsc::Receiver<()>, thread::JoinHandle<()>) {
    let (hotkey_tx, hotkey_rx) = channel();
    let hotkey_handle = crate::gui::hotkey::start_hotkey_listener(move || {
        let _ = hotkey_tx.send(());
    });
    (hotkey_rx, hotkey_handle)
}

fn start_clipboard_forwarder(
    clipboard_rx: mpsc::Receiver<String>,
    shared_records: Arc<Mutex<Vec<ClipboardRecord>>>,
    repository: Option<Arc<ClipboardRepository>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while let Ok(new) = clipboard_rx.recv() {
            if let Some(ref repo) = repository {
                match repo.save_text_if_not_duplicate(new.clone()) {
                    Ok(Some(record)) => {
                        let mut guard = match shared_records.lock() {
                            Ok(g) => g,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        guard.insert(0, record);
                        if guard.len() > 50 {
                            guard.truncate(50);
                        }
                    }
                    Ok(None) => {
                        println!("[ropy] 剪切板内容重复，跳过保存");
                    }
                    Err(e) => {
                        eprintln!("[ropy] 保存剪切板记录失败: {}", e);
                    }
                }
            }
        }
    })
}

fn create_window(
    cx: &mut App,
    shared_records: Arc<Mutex<Vec<ClipboardRecord>>>,
    is_visible: Arc<AtomicBool>,
) -> WindowHandle<RopyBoard> {
    let bounds = Bounds::centered(None, size(px(400.), px(600.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::PopUp,
            ..Default::default()
        },
        |_, cx| cx.new(|_| RopyBoard::new(shared_records, is_visible)),
    )
    .unwrap()
}

fn start_hotkey_handler(
    hotkey_rx: mpsc::Receiver<()>,
    window_handle: WindowHandle<RopyBoard>,
    async_app: AsyncApp,
) {
    let fg_executor = async_app.foreground_executor().clone();
    let bg_executor = async_app.background_executor().clone();
    fg_executor
        .spawn(async move {
            loop {
                while let Ok(()) = hotkey_rx.try_recv() {
                    let _ = async_app.update(move |cx| {
                        window_handle
                            .update(cx, |board, _window, cx| {
                                board.toggle_visibility(cx);
                            })
                            .ok();
                    });
                }
                bg_executor.timer(Duration::from_millis(16)).await;
            }
        })
        .detach();
}

pub fn launch_app() {
    Application::new().run(|cx: &mut App| {
        #[cfg(target_os = "macos")]
        set_activation_policy_accessory();

        let repository = initialize_repository();
        let initial_records = load_initial_records(&repository);
        let shared_records = Arc::new(Mutex::new(initial_records));
        let is_visible = Arc::new(AtomicBool::new(true));
        let (clipboard_rx, _listener_handle) = start_clipboard_monitor();
        let (hotkey_rx, _hotkey_handle) = start_hotkey_monitor();
        let _forwarder_handle =
            start_clipboard_forwarder(clipboard_rx, shared_records.clone(), repository.clone());
        let window_handle = create_window(cx, shared_records, is_visible.clone());
        let async_app = cx.to_async();
        start_hotkey_handler(hotkey_rx, window_handle, async_app);
        cx.activate(true);
    });
}

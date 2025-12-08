use crate::clipboard;
use crate::gui::board::RopyBoard;
use crate::repository::{ClipboardRecord, ClipboardRepository};
use gpui::{
    AnyWindowHandle, App, AppContext, Application, AsyncApp, BackgroundExecutor, Bounds,
    ForegroundExecutor, WindowBounds, WindowKind, WindowOptions, px, size,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{
    Arc, Mutex,
    mpsc::{self, channel},
};
use std::thread;
use std::time::Duration;

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
) -> AnyWindowHandle {
    let bounds = Bounds::centered(None, size(px(400.), px(600.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::PopUp,
            ..Default::default()
        },
        |_, cx| cx.new(|_| RopyBoard::new(shared_records)),
    )
    .unwrap()
    .into()
}

fn start_hotkey_handler(
    hotkey_rx: mpsc::Receiver<()>,
    is_visible: Arc<AtomicBool>,
    async_app: AsyncApp,
    fg_executor: ForegroundExecutor,
    bg_executor: BackgroundExecutor,
) {
    fg_executor
        .spawn(async move {
            loop {
                while let Ok(()) = hotkey_rx.try_recv() {
                    let current_visible = is_visible.load(Ordering::SeqCst);
                    let new_visible = !current_visible;
                    is_visible.store(new_visible, Ordering::SeqCst);

                    let _ = async_app.update(|app_cx| {
                        if new_visible {
                            app_cx.activate(true);
                        } else {
                            app_cx.hide();
                        }
                    });
                }
                bg_executor.timer(Duration::from_millis(16)).await;
            }
        })
        .detach();
}

pub fn launch_app() {
    Application::new().run(|cx: &mut App| {
        let repository = initialize_repository();
        let initial_records = load_initial_records(&repository);
        let shared_records = Arc::new(Mutex::new(initial_records));
        let (clipboard_rx, _listener_handle) = start_clipboard_monitor();
        let (hotkey_rx, _hotkey_handle) = start_hotkey_monitor();
        let _forwarder_handle =
            start_clipboard_forwarder(clipboard_rx, shared_records.clone(), repository.clone());
        let is_visible = Arc::new(AtomicBool::new(true));
        let _window_handle = create_window(cx, shared_records);
        let async_app = cx.to_async();
        let fg_executor = cx.foreground_executor().clone();
        let bg_executor = cx.background_executor().clone();
        start_hotkey_handler(hotkey_rx, is_visible, async_app, fg_executor, bg_executor);

        cx.activate(true);
    });
}

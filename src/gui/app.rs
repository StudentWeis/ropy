use crate::clipboard;
use crate::gui::board::{RopyBoard, active_window};
use crate::repository::{ClipboardRecord, ClipboardRepository};
use gpui::{
    App, AppContext, Application, AsyncApp, Bounds, KeyBinding, WindowBounds, WindowHandle,
    WindowKind, WindowOptions, px, size,
};
use gpui_component::Root;
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
        // Config the app to be accessory (no dock icon & cmd tab)
        let _succeeded: bool = msg_send![app, setActivationPolicy: 1isize];
    }
}

fn initialize_repository() -> Option<Arc<ClipboardRepository>> {
    match ClipboardRepository::new() {
        Ok(repo) => {
            println!("[ropy] Clipboard history repository initialized");
            Some(Arc::new(repo))
        }
        Err(e) => {
            eprintln!("[ropy] Clipboard repository initialization failed: {e}");
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
                            repo.cleanup_old_records(50).ok();
                        }
                    }
                    Ok(None) => {
                        println!("[ropy] Content is duplicate, not saving.");
                    }
                    Err(e) => {
                        eprintln!("[ropy] Failed to save clipboard record: {e}");
                    }
                }
            }
        }
    })
}

fn create_window(
    cx: &mut App,
    shared_records: Arc<Mutex<Vec<ClipboardRecord>>>,
    repository: Option<Arc<ClipboardRepository>>,
) -> WindowHandle<Root> {
    let bounds = Bounds::centered(None, size(px(400.), px(600.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            kind: WindowKind::PopUp,
            titlebar: None,
            ..Default::default()
        },
        |window, cx| {
            gpui_component::theme::Theme::change(gpui_component::theme::ThemeMode::Dark, Some(window), cx);
            let view = cx.new(|cx| RopyBoard::new(shared_records, repository.clone(), window, cx));
            cx.new(|cx| Root::new(view, window, cx))
        },
    )
    .unwrap()
}

fn start_hotkey_handler(
    hotkey_rx: mpsc::Receiver<()>,
    window_handle: WindowHandle<Root>,
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
                            .update(cx, |_, window, cx| active_window(window, cx))
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
        // Initialize gpui-component
        gpui_component::init(cx);

        // Set activation policy on macOS
        #[cfg(target_os = "macos")]
        set_activation_policy_accessory();

        cx.bind_keys([
            KeyBinding::new("escape", crate::gui::board::Hide, None),
            #[cfg(target_os = "macos")]
            KeyBinding::new("cmd-q", crate::gui::board::Quit, None),
            #[cfg(target_os = "windows")]
            KeyBinding::new("alt-f4", crate::gui::board::Quit, None),
        ]);

        let repository = initialize_repository();
        let initial_records = load_initial_records(&repository);
        let shared_records = Arc::new(Mutex::new(initial_records));
        let (clipboard_rx, _listener_handle) = start_clipboard_monitor();
        let (hotkey_rx, _hotkey_handle) = start_hotkey_monitor();
        let _forwarder_handle =
            start_clipboard_forwarder(clipboard_rx, shared_records.clone(), repository.clone());
        let window_handle = create_window(cx, shared_records, repository.clone());
        let async_app = cx.to_async();
        start_hotkey_handler(hotkey_rx, window_handle, async_app);
        cx.activate(true);
    });
}

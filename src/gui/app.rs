use crate::clipboard;
use crate::repository::ClipboardRepository;
use gpui::{
    AnyWindowHandle, App, AppContext, Application, Bounds, Context, Window, WindowBounds,
    WindowKind, WindowOptions, div, prelude::*, px, rgb, size,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc::channel};
use std::time::Duration;

struct RopyBoard {
    text: Arc<Mutex<String>>,
}

impl Render for RopyBoard {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let text_guard = self.text.lock().unwrap();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .bg(rgb(0x505050))
            .size(px(500.0))
            .justify_center()
            .items_center()
            .shadow_lg()
            .border_1()
            .border_color(rgb(0x0000ff))
            .text_xl()
            .text_color(rgb(0xffffff))
            .child((*text_guard).to_string())
    }
}

pub fn launch_app() {
    Application::new().run(|cx: &mut App| {
        let shared_text: Arc<Mutex<String>> = Arc::new(Mutex::new("World".to_string()));
        let (clipboard_tx, clipboard_rx) = channel::<String>();
        let _listener_handle =
            clipboard::start_clipboard_monitor(clipboard_tx, Duration::from_millis(300));

        // 初始化剪切板历史记录仓库
        let repository = match ClipboardRepository::new() {
            Ok(repo) => {
                println!("[ropy] 剪切板历史记录仓库初始化成功");
                Some(Arc::new(repo))
            }
            Err(e) => {
                eprintln!("[ropy] 剪切板历史记录仓库初始化失败: {}", e);
                None
            }
        };

        // Channel for hotkey toggle events
        let (hotkey_tx, hotkey_rx) = channel();
        let is_visible = Arc::new(AtomicBool::new(true));

        // Start global hotkey listener (SHIFT + D) to send toggle events
        let _hotkey_handle = crate::gui::hotkey::start_hotkey_listener(move || {
            let _ = hotkey_tx.send(());
        });

        // Forward clipboard messages to the shared string on a short-lived thread so we don't block the UI
        let ui_text_clone = shared_text.clone();
        let repo_clone = repository.clone();
        let _forwarder_handle = std::thread::spawn(move || {
            while let Ok(new) = clipboard_rx.recv() {
                // 保存到数据库（带去重检查）
                if let Some(ref repo) = repo_clone {
                    match repo.save_text_if_not_duplicate(new.clone()) {
                        Ok(Some(record)) => {
                            println!("[ropy] 剪切板记录已保存, id: {}", record.id);
                        }
                        Ok(None) => {
                            println!("[ropy] 剪切板内容重复，跳过保存");
                        }
                        Err(e) => {
                            eprintln!("[ropy] 保存剪切板记录失败: {}", e);
                        }
                    }
                }

                // 更新 UI 显示
                if let Ok(mut guard) = ui_text_clone.lock() {
                    *guard = new;
                }
            }
        });

        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        let _window_handle: AnyWindowHandle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    // Use PopUp kind to make window float above others
                    kind: WindowKind::PopUp,
                    ..Default::default()
                },
                |_, cx| {
                    cx.new(|_| RopyBoard {
                        text: shared_text.clone(),
                    })
                },
            )
            .unwrap()
            .into();

        // Get AsyncApp for updating
        let async_app = cx.to_async();
        let fg_executor = cx.foreground_executor().clone();
        let bg_executor = cx.background_executor().clone();

        // Use a crossbeam channel or async-compatible channel to wake up the app
        // For now, we'll use the foreground executor with a polling approach
        fg_executor
            .spawn(async move {
                loop {
                    // Poll for hotkey events
                    while let Ok(()) = hotkey_rx.try_recv() {
                        let current_visible = is_visible.load(Ordering::SeqCst);
                        let new_visible = !current_visible;
                        is_visible.store(new_visible, Ordering::SeqCst);

                        // Update app visibility
                        let _ = async_app.update(|app_cx| {
                            if new_visible {
                                // Show: activate the app
                                app_cx.activate(true);
                            } else {
                                // Hide: hide the app
                                app_cx.hide();
                            }
                        });
                    }
                    // Timer to keep polling
                    bg_executor.timer(Duration::from_millis(16)).await;
                }
            })
            .detach();

        cx.activate(true);
    });
}

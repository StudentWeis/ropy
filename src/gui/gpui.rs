use crate::clipboard;
use gpui::{
    App, Application, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px,
    rgb, size,
};
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
        let (tx, rx) = channel::<String>();
        let _listener_handle = clipboard::spawn_clipboard_listener(tx, Duration::from_millis(300));

        // Forward clipboard messages to the shared string on a short-lived thread so we don't block the UI
        let ui_text_clone = shared_text.clone();
        let _forwarder_handle = std::thread::spawn(move || {
            while let Ok(new) = rx.recv() {
                if let Ok(mut guard) = ui_text_clone.lock() {
                    *guard = new;
                }
            }
        });

        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| RopyBoard {
                    text: shared_text.clone(),
                })
            },
        )
        .unwrap();
        cx.activate(true);
    });
}

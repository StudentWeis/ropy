use std::sync::mpsc::Sender as CompletionSender;

mod listener;
mod utils;
mod writer;

pub use listener::{start_clipboard_listener, start_clipboard_monitor};
pub use utils::save_image;
pub use writer::start_clipboard_writer;

pub enum ClipboardEvent {
    Text(String),
    /// Image(path, `content_hash`)
    Image(String, u64),
}

pub enum CopyRequest {
    Text {
        text: String,
        completion: Option<CompletionSender<()>>,
    },
    Image {
        path: String,
        completion: Option<CompletionSender<()>>,
    },
}

impl CopyRequest {
    pub const fn text(text: String) -> Self {
        Self::Text {
            text,
            completion: None,
        }
    }

    pub const fn text_with_completion(text: String, completion: CompletionSender<()>) -> Self {
        Self::Text {
            text,
            completion: Some(completion),
        }
    }

    pub const fn image(path: String) -> Self {
        Self::Image {
            path,
            completion: None,
        }
    }

    pub const fn image_with_completion(path: String, completion: CompletionSender<()>) -> Self {
        Self::Image {
            path,
            completion: Some(completion),
        }
    }
}

pub enum LastCopyState {
    Text(String),
    Image(u64),
}

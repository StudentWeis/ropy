use std::sync::mpsc::Sender as CompletionSender;

mod listener;
mod utils;
mod writer;

pub use listener::start_clipboard_monitor;
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

#[cfg(test)]
mod tests {
    use std::{sync::mpsc, time::Duration};

    use super::*;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_copy_request_text_constructor_sets_text_without_completion() {
        let request = CopyRequest::text("hello".to_string());

        match request {
            CopyRequest::Text { text, completion } => {
                assert_eq!(text, "hello");
                assert!(completion.is_none());
            }
            CopyRequest::Image { .. } => panic!("expected text copy request"),
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_copy_request_text_with_completion_sends_signal() {
        let (completion_tx, completion_rx) = mpsc::channel();
        let request = CopyRequest::text_with_completion("hello".to_string(), completion_tx);

        match request {
            CopyRequest::Text { text, completion } => {
                assert_eq!(text, "hello");
                completion.unwrap().send(()).unwrap();
            }
            CopyRequest::Image { .. } => panic!("expected text copy request"),
        }

        assert_eq!(completion_rx.recv_timeout(Duration::from_secs(1)), Ok(()));
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_copy_request_image_constructor_sets_path_without_completion() {
        let request = CopyRequest::image("/tmp/example.png".to_string());

        match request {
            CopyRequest::Image { path, completion } => {
                assert_eq!(path, "/tmp/example.png");
                assert!(completion.is_none());
            }
            CopyRequest::Text { .. } => panic!("expected image copy request"),
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_copy_request_image_with_completion_sends_signal() {
        let (completion_tx, completion_rx) = mpsc::channel();
        let request =
            CopyRequest::image_with_completion("/tmp/example.png".to_string(), completion_tx);

        match request {
            CopyRequest::Image { path, completion } => {
                assert_eq!(path, "/tmp/example.png");
                completion.unwrap().send(()).unwrap();
            }
            CopyRequest::Text { .. } => panic!("expected image copy request"),
        }

        assert_eq!(completion_rx.recv_timeout(Duration::from_secs(1)), Ok(()));
    }
}

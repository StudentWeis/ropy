use std::sync::mpsc::Sender as CompletionSender;

mod listener;
mod utils;
mod writer;

pub use listener::start_clipboard_monitor;
pub use utils::{
    load_rich_text_html, load_rich_text_rtf, remove_rich_text_files, save_image,
    save_rich_text_files_to_dir, thumb_path_for,
};
pub use writer::start_clipboard_writer;

pub enum ClipboardEvent {
    Text(String),
    /// Image(path, `content_hash`)
    Image(String, u64),
    Files(Vec<String>),
    RichText {
        plain_text: String,
        html: Option<String>,
        rtf: Option<String>,
    },
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
    Files {
        paths: Vec<String>,
        completion: Option<CompletionSender<()>>,
    },
    RichText {
        plain_text: String,
        html: Option<String>,
        rtf: Option<String>,
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

    pub const fn files(paths: Vec<String>) -> Self {
        Self::Files {
            paths,
            completion: None,
        }
    }

    pub const fn files_with_completion(
        paths: Vec<String>,
        completion: CompletionSender<()>,
    ) -> Self {
        Self::Files {
            paths,
            completion: Some(completion),
        }
    }

    pub const fn rich_text(plain_text: String, html: Option<String>, rtf: Option<String>) -> Self {
        Self::RichText {
            plain_text,
            html,
            rtf,
            completion: None,
        }
    }

    pub const fn rich_text_with_completion(
        plain_text: String,
        html: Option<String>,
        rtf: Option<String>,
        completion: CompletionSender<()>,
    ) -> Self {
        Self::RichText {
            plain_text,
            html,
            rtf,
            completion: Some(completion),
        }
    }
}

pub enum LastCopyState {
    Text(String),
    Image(u64),
    Files(u64),
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
            CopyRequest::Image { .. }
            | CopyRequest::Files { .. }
            | CopyRequest::RichText { .. } => {
                panic!("expected text copy request")
            }
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
            CopyRequest::Image { .. }
            | CopyRequest::Files { .. }
            | CopyRequest::RichText { .. } => {
                panic!("expected text copy request")
            }
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
            CopyRequest::Text { .. } | CopyRequest::Files { .. } | CopyRequest::RichText { .. } => {
                panic!("expected image copy request")
            }
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
            CopyRequest::Text { .. } | CopyRequest::Files { .. } | CopyRequest::RichText { .. } => {
                panic!("expected image copy request")
            }
        }

        assert_eq!(completion_rx.recv_timeout(Duration::from_secs(1)), Ok(()));
    }

    #[test]
    fn test_copy_request_files_constructor_sets_paths_without_completion() {
        let request = CopyRequest::files(vec!["/tmp/example.txt".to_string()]);

        match request {
            CopyRequest::Files { paths, completion } => {
                assert_eq!(paths, vec!["/tmp/example.txt"]);
                assert!(completion.is_none());
            }
            CopyRequest::Text { .. } | CopyRequest::Image { .. } | CopyRequest::RichText { .. } => {
                panic!("expected files copy request")
            }
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_copy_request_files_with_completion_sends_signal() {
        let (completion_tx, completion_rx) = mpsc::channel();
        let request =
            CopyRequest::files_with_completion(vec!["/tmp/example.txt".to_string()], completion_tx);

        match request {
            CopyRequest::Files { paths, completion } => {
                assert_eq!(paths, vec!["/tmp/example.txt"]);
                completion.unwrap().send(()).unwrap();
            }
            CopyRequest::Text { .. } | CopyRequest::Image { .. } | CopyRequest::RichText { .. } => {
                panic!("expected files copy request")
            }
        }

        assert_eq!(completion_rx.recv_timeout(Duration::from_secs(1)), Ok(()));
    }

    #[test]
    fn test_copy_request_rich_text_constructor_sets_payload_without_completion() {
        let request = CopyRequest::rich_text(
            "hello".to_string(),
            Some("<p>hello</p>".to_string()),
            Some("{\\rtf1 hello}".to_string()),
        );

        match request {
            CopyRequest::RichText {
                plain_text,
                html,
                rtf,
                completion,
            } => {
                assert_eq!(plain_text, "hello");
                assert_eq!(html.as_deref(), Some("<p>hello</p>"));
                assert_eq!(rtf.as_deref(), Some("{\\rtf1 hello}"));
                assert!(completion.is_none());
            }
            CopyRequest::Text { .. } | CopyRequest::Image { .. } | CopyRequest::Files { .. } => {
                panic!("expected rich text copy request")
            }
        }
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_copy_request_rich_text_with_completion_sends_signal() {
        let (completion_tx, completion_rx) = mpsc::channel();
        let request = CopyRequest::rich_text_with_completion(
            "hello".to_string(),
            Some("<p>hello</p>".to_string()),
            Some("{\\rtf1 hello}".to_string()),
            completion_tx,
        );

        match request {
            CopyRequest::RichText {
                plain_text,
                html,
                rtf,
                completion,
            } => {
                assert_eq!(plain_text, "hello");
                assert_eq!(html.as_deref(), Some("<p>hello</p>"));
                assert_eq!(rtf.as_deref(), Some("{\\rtf1 hello}"));
                completion.unwrap().send(()).unwrap();
            }
            CopyRequest::Text { .. } | CopyRequest::Image { .. } | CopyRequest::Files { .. } => {
                panic!("expected rich text copy request")
            }
        }

        assert_eq!(completion_rx.recv_timeout(Duration::from_secs(1)), Ok(()));
    }
}

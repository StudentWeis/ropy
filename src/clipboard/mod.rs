mod listener;
mod utils;
mod writer;

pub use listener::start_clipboard_listener;
pub use listener::start_clipboard_monitor;
pub use utils::save_image;
pub use writer::start_clipboard_writer;

pub enum ClipboardEvent {
    Text(String),
    Image(String),
}

pub enum CopyRequest {
    Text(String),
    Image(String),
}

pub enum LastCopyState {
    Text(String),
    Image(u64),
}

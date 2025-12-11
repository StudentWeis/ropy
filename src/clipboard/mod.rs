mod listener;
mod writer;

pub use listener::start_clipboard_monitor;
pub use writer::set_image as copy_image;
pub use writer::set_text as copy_text;

pub enum ClipboardEvent {
    Text(String),
    Image(String),
}

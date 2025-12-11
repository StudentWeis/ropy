mod listener;
mod writer;

pub use listener::start_clipboard_monitor;
pub use writer::set_text as copy_text;

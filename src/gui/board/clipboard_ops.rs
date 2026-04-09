use std::{sync::mpsc, time::Duration};

use gpui::{Context, Window};

use super::RopyBoard;
use crate::{
    clipboard::{CopyRequest, load_rich_text_html, load_rich_text_rtf},
    config::ConfirmMode,
    gui::{hide_window, paste},
    repository::{ClipboardRecord, models::ContentType},
    utils::{deserialize_file_paths, read_or_recover},
};

const CLIPBOARD_WRITE_COMPLETION_TIMEOUT_MS: u64 = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(in crate::gui::board) enum ConfirmFormat {
    #[default]
    Default,
    PlainText,
}

pub(in crate::gui::board) fn build_copy_request(
    content: &str,
    content_type: &ContentType,
    completion: Option<mpsc::Sender<()>>,
) -> Option<CopyRequest> {
    match content_type {
        ContentType::Text => Some(completion.map_or_else(
            || CopyRequest::text(content.to_string()),
            |tx| CopyRequest::text_with_completion(content.to_string(), tx),
        )),
        ContentType::Image => Some(completion.map_or_else(
            || CopyRequest::image(content.to_string()),
            |tx| CopyRequest::image_with_completion(content.to_string(), tx),
        )),
        ContentType::FilePath => {
            let paths = deserialize_file_paths(content);
            if paths.is_empty() {
                None
            } else {
                Some(if let Some(tx) = completion {
                    CopyRequest::files_with_completion(paths, tx)
                } else {
                    CopyRequest::files(paths)
                })
            }
        }
        ContentType::RichText => Some(completion.map_or_else(
            || CopyRequest::rich_text(content.to_string(), None, None),
            |tx| CopyRequest::rich_text_with_completion(content.to_string(), None, None, tx),
        )),
    }
}

pub(in crate::gui::board) fn build_copy_request_for_record(
    record: &ClipboardRecord,
    confirm_format: ConfirmFormat,
    completion: Option<mpsc::Sender<()>>,
) -> Option<CopyRequest> {
    if confirm_format == ConfirmFormat::PlainText && record.content_type == ContentType::RichText {
        return Some(completion.map_or_else(
            || CopyRequest::text(record.content.clone()),
            |tx| CopyRequest::text_with_completion(record.content.clone(), tx),
        ));
    }

    if record.content_type != ContentType::RichText {
        return build_copy_request(&record.content, &record.content_type, completion);
    }

    let (html, rtf) = record.rich_text_meta.as_ref().map_or((None, None), |meta| {
        (load_rich_text_html(meta), load_rich_text_rtf(meta))
    });

    Some(match completion {
        Some(tx) => CopyRequest::rich_text_with_completion(record.content.clone(), html, rtf, tx),
        None => CopyRequest::rich_text(record.content.clone(), html, rtf),
    })
}

impl RopyBoard {
    /// Write confirmed content to the clipboard before the confirm action completes.
    pub(super) fn write_content_to_clipboard(
        &self,
        record: &ClipboardRecord,
        confirm_format: ConfirmFormat,
    ) -> bool {
        let completion = self
            .confirm_mode
            .requires_clipboard_completion()
            .then(mpsc::channel);
        let request = build_copy_request_for_record(
            record,
            confirm_format,
            completion.as_ref().map(|(tx, _)| tx.clone()),
        );

        if let Some(req) = request {
            if self.copy_tx.send_blocking(req).is_err() {
                tracing::warn!("failed to send clipboard write request");
                return false;
            }
            if let Some((_, rx)) = completion
                && rx
                    .recv_timeout(Duration::from_millis(CLIPBOARD_WRITE_COMPLETION_TIMEOUT_MS))
                    .is_err()
            {
                tracing::warn!("timed out waiting for clipboard write completion");
                return false;
            }
            return true;
        }

        false
    }

    /// Confirm selection: copy record to clipboard and hide.
    /// The clipboard listener will re-capture the copy event and the
    /// repository layer handles deduplication via content hash upsert.
    pub(crate) fn confirm_record(&self, window: &mut Window, cx: &Context<Self>, index: usize) {
        self.confirm_record_with_format(window, cx, index, ConfirmFormat::Default);
    }

    pub(crate) fn confirm_record_as_plain_text(
        &self,
        window: &mut Window,
        cx: &Context<Self>,
        index: usize,
    ) {
        self.confirm_record_with_format(window, cx, index, ConfirmFormat::PlainText);
    }

    fn confirm_record_with_format(
        &self,
        window: &mut Window,
        cx: &Context<Self>,
        index: usize,
        confirm_format: ConfirmFormat,
    ) {
        let record = {
            let Some(record_index) = self.filtered_record_index_at(index) else {
                return;
            };
            let record = {
                let records = read_or_recover(&self.records);
                records.get(record_index).cloned()
            };
            let Some(record) = record else {
                tracing::warn!(
                    index = record_index,
                    "failed to resolve filtered record from cache"
                );
                return;
            };
            record
        };

        if !self.write_content_to_clipboard(&record, confirm_format) {
            return;
        }

        match self.confirm_mode {
            ConfirmMode::CopyToClipboard => {
                if !self.pinned {
                    hide_window(window, cx, self.pinned);
                }
            }
            ConfirmMode::PasteImmediately => {
                hide_window(window, cx, false);
                if let Err(error) = paste::trigger_paste() {
                    tracing::warn!(error = %error, "failed to trigger immediate paste");
                }
            }
        }
    }
}

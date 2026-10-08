use std::{future::poll_fn, pin::pin, task::Poll, time::Duration};

use gpui_kit::{AppContext, BackgroundExecutor, Context, Window};

use super::RopyBoard;
use crate::{
    clipboard::{ClipboardWriteResult, CopyRequest, load_rich_text_html, load_rich_text_rtf},
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
    completion: Option<async_channel::Sender<ClipboardWriteResult>>,
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
    completion: Option<async_channel::Sender<ClipboardWriteResult>>,
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

pub(in crate::gui::board) async fn wait_for_clipboard_write(
    rx: &async_channel::Receiver<ClipboardWriteResult>,
    executor: &BackgroundExecutor,
) -> bool {
    let mut result = pin!(rx.recv());
    let mut timeout =
        pin!(executor.timer(Duration::from_millis(CLIPBOARD_WRITE_COMPLETION_TIMEOUT_MS)));
    poll_fn(|cx| {
        if let Poll::Ready(result) = result.as_mut().poll(cx) {
            return Poll::Ready(match result {
                Ok(Ok(())) => true,
                error => {
                    tracing::warn!(?error, "clipboard write failed");
                    false
                }
            });
        }
        if timeout.as_mut().poll(cx).is_ready() {
            tracing::warn!("timed out waiting for clipboard write completion");
            return Poll::Ready(false);
        }
        Poll::Pending
    })
    .await
}

impl RopyBoard {
    /// Confirm selection: copy record to clipboard and hide.
    /// The clipboard listener will re-capture the copy event and the
    /// repository layer handles deduplication via content hash upsert.
    pub(crate) fn confirm_record(
        &mut self,
        window: &Window,
        cx: &mut Context<'_, Self>,
        index: usize,
    ) {
        self.confirm_record_with_format(window, cx, index, ConfirmFormat::Default);
    }

    pub(crate) fn confirm_record_as_plain_text(
        &mut self,
        window: &Window,
        cx: &mut Context<'_, Self>,
        index: usize,
    ) {
        self.confirm_record_with_format(window, cx, index, ConfirmFormat::PlainText);
    }

    fn confirm_record_with_format(
        &mut self,
        window: &Window,
        cx: &mut Context<'_, Self>,
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

        if self.copy_in_progress {
            return;
        }
        self.copy_in_progress = true;
        self.copy_generation = self.copy_generation.wrapping_add(1);
        let generation = self.copy_generation;
        let mode = self.confirm_mode;
        let copy_tx = self.copy_tx.clone();
        // Rich-text sidecars are read off the UI thread.
        let request = cx.background_spawn(async move {
            let completion = mode
                .requires_clipboard_completion()
                .then(|| async_channel::bounded(1));
            let request = build_copy_request_for_record(
                &record,
                confirm_format,
                completion.as_ref().map(|(tx, _)| tx.clone()),
            );
            (request, completion.map(|(_, rx)| rx))
        });
        cx.spawn_in(window, async move |this, cx| {
            let (request, completion) = request.await;
            let success = if let Some(request) = request {
                if copy_tx.send(request).await.is_err() {
                    tracing::warn!("failed to send clipboard write request");
                    false
                } else if let Some(rx) = completion {
                    wait_for_clipboard_write(&rx, cx.background_executor()).await
                } else {
                    true
                }
            } else {
                false
            };
            let _ = this.update_in(cx, |this, window, cx| {
                this.copy_in_progress = false;
                cx.notify();
                // A dismissed/reopened board must not hide or paste on a stale completion.
                if !success
                    || this.copy_generation != generation
                    || this.active_panel != super::ActivePanel::ClipboardList
                    || this.ui_state.any_overlay_visible()
                {
                    return;
                }
                match mode {
                    ConfirmMode::CopyToClipboard => {
                        if !this.pinned {
                            hide_window(window, cx, this.pinned);
                        }
                    }
                    ConfirmMode::PasteImmediately => {
                        hide_window(window, cx, false);
                        cx.background_spawn(async {
                            if let Err(error) = paste::trigger_paste() {
                                tracing::warn!(error = %error, "failed to trigger immediate paste");
                            }
                        })
                        .detach();
                    }
                }
            });
        })
        .detach();
        cx.notify();
    }
}

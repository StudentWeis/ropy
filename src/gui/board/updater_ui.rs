use gpui::Context;

use super::RopyBoard;
use crate::{config::Settings, updater::models::UpdateStatus};

impl RopyBoard {
    /// Trigger a manual update check in the background
    pub fn check_for_update_async(&mut self, cx: &mut Context<Self>) {
        self.update_manager.status = UpdateStatus::Checking;
        cx.notify();

        let include_prerelease = Settings::read(cx, |s| s.update.include_prerelease);

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    // Use std::thread::spawn to run blocking operation
                    let (tx, rx) = std::sync::mpsc::channel();
                    let _handle = std::thread::spawn(move || {
                        let update_result =
                            crate::updater::checker::check_for_update(include_prerelease);
                        let _ = tx.send(update_result);
                    });

                    rx.recv().unwrap_or_else(|_| {
                        Err(crate::updater::errors::UpdateError::Network(
                            "Update check failed".to_string(),
                        ))
                    })
                })
                .await;

            let _ = this.update(cx, |board, cx| {
                match result {
                    Ok(Some(info)) => {
                        board.update_manager.status = UpdateStatus::Available(info);
                    }
                    Ok(None) => {
                        board.update_manager.status = UpdateStatus::UpToDate;
                    }
                    Err(e) => {
                        tracing::warn!(error = ?e, "update check failed");
                        board.update_manager.status = UpdateStatus::Error(e.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Trigger download and install in the background with progress updates
    pub fn download_and_install_update(&mut self, cx: &mut Context<Self>) {
        let release = match &self.update_manager.status {
            UpdateStatus::Available(info) => info.clone(),
            _ => return,
        };
        self.update_manager.status = UpdateStatus::Downloading(0.0);
        cx.notify();

        let (progress_tx, progress_rx) = async_channel::unbounded::<f32>();
        let (result_tx, result_rx) =
            async_channel::bounded::<Result<(), crate::updater::errors::UpdateError>>(1);

        // Launch the blocking download on a dedicated OS thread.
        // progress_tx is moved into the thread; when it is dropped
        // (download completes or fails) the progress channel closes automatically.
        std::thread::spawn(move || {
            let update_result =
                crate::updater::downloader::download_and_install(&release, &progress_tx);
            let _ = result_tx.send_blocking(update_result);
        });

        cx.spawn(async move |this, cx| {
            // Listen for progress updates on the foreground thread and
            // refresh the UI in real time. The loop exits when the sender
            // is dropped (download finishes or fails).
            while let Ok(progress) = progress_rx.recv().await {
                let _ = this.update(cx, |board, cx| {
                    board.update_manager.status = UpdateStatus::Downloading(progress);
                    cx.notify();
                });
            }

            // Download is done – collect the result and update final status
            let result = result_rx.recv().await.unwrap_or_else(|_| {
                Err(crate::updater::errors::UpdateError::Network(
                    "Update installation failed".to_string(),
                ))
            });

            let _ = this.update(cx, |board, cx| {
                match result {
                    Ok(()) => {
                        board.update_manager.status = UpdateStatus::ReadyToRestart;
                    }
                    Err(e) => {
                        tracing::error!(error = ?e, "update installation failed");
                        board.update_manager.status = UpdateStatus::Error(e.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

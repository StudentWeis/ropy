use gpui::Context;

use super::RopyBoard;
use crate::{config::Settings, updater::models::UpdateStatus};

impl RopyBoard {
    /// Trigger a manual update check in the background
    pub fn check_for_update_async(&mut self, cx: &mut Context<Self>) {
        self.update_status = UpdateStatus::Checking;
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
                        board.update_status = UpdateStatus::Available(info);
                    }
                    Ok(None) => {
                        board.update_status = UpdateStatus::UpToDate;
                    }
                    Err(e) => {
                        tracing::warn!(error = ?e, "update check failed");
                        board.update_status = UpdateStatus::Error(e.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Trigger download and install in the background
    pub fn download_and_install_update(&mut self, cx: &mut Context<Self>) {
        let release = match &self.update_status {
            UpdateStatus::Available(info) => info.clone(),
            _ => return,
        };
        self.update_status = UpdateStatus::Downloading(0.0);
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    // Use std::thread::spawn to run blocking operation
                    let (tx, rx) = std::sync::mpsc::channel();
                    let _handle = std::thread::spawn(move || {
                        let update_result = crate::updater::downloader::download_and_install(
                            &release,
                            |_progress| {
                                // Progress callback runs on blocking thread;
                                // mid-download UI updates are skipped for simplicity.
                            },
                        );
                        let _ = tx.send(update_result);
                    });

                    rx.recv().unwrap_or_else(|_| {
                        Err(crate::updater::errors::UpdateError::Network(
                            "Update installation failed".to_string(),
                        ))
                    })
                })
                .await;

            let _ = this.update(cx, |board, cx| {
                match result {
                    Ok(()) => {
                        board.update_status = UpdateStatus::ReadyToRestart;
                    }
                    Err(e) => {
                        tracing::error!(error = ?e, "update installation failed");
                        board.update_status = UpdateStatus::Error(e.to_string());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

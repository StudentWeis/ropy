use gpui::{AppContext as _, Context};

use super::RopyBoard;
use crate::{
    config::Settings,
    updater::{errors::UpdateError, models::UpdateStatus},
};

impl RopyBoard {
    pub fn check_for_update_async(&mut self, cx: &mut Context<Self>) {
        self.update_manager.status = UpdateStatus::Checking;
        cx.notify();

        let include_prerelease = Settings::read(cx, |s| s.update.include_prerelease);
        let background_task = cx.background_spawn(async move {
            crate::updater::checker::check_for_update(include_prerelease)
        });

        cx.spawn(async move |this, cx| {
            let result: Result<_, UpdateError> = background_task.await;

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

    pub fn download_and_install_update(&mut self, cx: &mut Context<Self>) {
        let release = match &self.update_manager.status {
            UpdateStatus::Available(info) => info.clone(),
            _ => return,
        };
        self.update_manager.status = UpdateStatus::Downloading(0.0);
        cx.notify();

        // download_and_install reports incremental progress via the
        // sender, so we pair it with a foreground listener below.
        let (progress_tx, progress_rx) = async_channel::unbounded::<f32>();

        let download_task = cx.background_spawn(async move {
            crate::updater::downloader::download_and_install(&release, &progress_tx)
        });

        cx.spawn(async move |this, cx| {
            // Loop exits naturally once the background task drops the
            // sender (download succeeds or fails), which is what lets us
            // collect the final result on the next line.
            while let Ok(progress) = progress_rx.recv().await {
                let _ = this.update(cx, |board, cx| {
                    board.update_manager.status = UpdateStatus::Downloading(progress);
                    cx.notify();
                });
            }

            let result: Result<(), UpdateError> = download_task.await;

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

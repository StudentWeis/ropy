use gpui_kit::{AppContext as _, Context};

use super::RopyBoard;
use crate::{
    gui::settings::GlobalSettings,
    updater::{errors::UpdateError, models::UpdateStatus},
};

impl RopyBoard {
    pub(crate) fn start_update_checks(&mut self, cx: &mut Context<'_, Self>) {
        if crate::updater::transaction::take_rollback_notice() {
            self.update_manager.status = UpdateStatus::RolledBack;
        } else if crate::updater::transaction::is_staged() {
            self.update_manager.status = UpdateStatus::ReadyToRestart;
        }
        self.check_for_update_if_due(cx);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(60))
                    .await;
                if this.update(cx, Self::check_for_update_if_due).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn check_for_update_if_due(&mut self, cx: &mut Context<'_, Self>) {
        if matches!(self.update_manager.status, UpdateStatus::RolledBack) {
            return;
        }
        let (enabled, prerelease) =
            GlobalSettings::read(cx, |s| (s.update.auto_check, s.update.include_prerelease));
        if enabled
            && (self
                .update_manager
                .schedule
                .is_due(crate::updater::schedule::now())
                || self.update_manager.schedule.channel_changed(prerelease))
        {
            self.check_for_update_async(cx);
        }
    }

    pub(crate) fn restart_after_update(&mut self, cx: &mut Context<'_, Self>) {
        if !matches!(
            self.update_manager.status,
            UpdateStatus::ReadyToRestart
                | UpdateStatus::Error(crate::updater::errors::UpdateFailure::Restart)
        ) {
            return;
        }
        self.update_manager.status = UpdateStatus::Restarting;
        cx.notify();
        let task = cx.background_spawn(async { crate::updater::transaction::start_restart() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |board, cx| match result {
                Ok(child) => {
                    board.update_manager.restart_child = Some(child);
                    cx.quit();
                }
                Err(error) => {
                    tracing::error!(%error, "restart handoff failed");
                    board.update_manager.status =
                        UpdateStatus::Error(crate::updater::errors::UpdateFailure::Restart);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn check_for_update_async(&mut self, cx: &mut Context<'_, Self>) {
        if !self.update_manager.begin_check() {
            return;
        }
        cx.notify();

        let include_prerelease = GlobalSettings::read(cx, |s| s.update.include_prerelease);
        let mut schedule = self.update_manager.schedule.clone();
        let background_task = cx.background_spawn(async move {
            let result = crate::updater::checker::check_for_update(include_prerelease);
            schedule.record(
                crate::updater::schedule::now(),
                result.is_ok(),
                include_prerelease,
            );
            schedule.save();
            (result, schedule)
        });

        cx.spawn(async move |this, cx| {
            let (result, schedule) = background_task.await;

            let _ = this.update(cx, |board, cx| {
                board.update_manager.schedule = schedule;
                // Changing channels during a request invalidates its result.
                if GlobalSettings::read(cx, |s| s.update.include_prerelease) != include_prerelease {
                    board.update_manager.status = UpdateStatus::Idle;
                    cx.notify();
                    return;
                }
                match result {
                    Ok(Some(info)) => {
                        board.update_manager.release = Some(info.clone());
                        board.update_manager.status = UpdateStatus::Available(info);
                    }
                    Ok(None) => {
                        board.update_manager.release = None;
                        board.update_manager.status = UpdateStatus::UpToDate;
                    }
                    Err(e) => {
                        tracing::warn!(error = ?e, "update check failed");
                        board.update_manager.status = UpdateStatus::Error((&e).into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn download_update(&mut self, cx: &mut Context<'_, Self>) {
        let release = match &self.update_manager.status {
            UpdateStatus::Available(info) => info.clone(),
            UpdateStatus::Error(_) => match &self.update_manager.release {
                Some(info) => info.clone(),
                None => return,
            },
            _ => return,
        };
        if !GlobalSettings::read(cx, |s| s.update.include_prerelease)
            && semver::Version::parse(&release.version).is_ok_and(|version| !version.pre.is_empty())
        {
            self.update_manager.release = None;
            self.update_manager.status = UpdateStatus::Idle;
            cx.notify();
            return;
        }
        self.update_manager.status = UpdateStatus::Downloading(0.0);
        cx.notify();

        // download_and_stage reports incremental progress via the
        // sender, so we pair it with a foreground listener below.
        let (progress_tx, progress_rx) = async_channel::bounded::<UpdateStatus>(8);

        let download_task = cx.background_spawn(async move {
            crate::updater::downloader::download_and_stage(&release, &progress_tx)
        });

        cx.spawn(async move |this, cx| {
            // Loop exits naturally once the background task drops the
            // sender (download succeeds or fails), which is what lets us
            // collect the final result on the next line.
            while let Ok(progress) = progress_rx.recv().await {
                let _ = this.update(cx, |board, cx| {
                    board.update_manager.status = progress;
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
                        board.update_manager.status = UpdateStatus::Error((&e).into());
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

//! Persistent automatic-check cadence, independent of GUI timers.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct CheckSchedule {
    last_check: u64,
    next_check: u64,
    failures: u32,
    prerelease: bool,
}

impl CheckSchedule {
    pub(crate) fn load() -> Self {
        Self::path()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub(crate) fn save(&self) {
        if let Some(path) = Self::path() {
            let result = (|| -> Result<(), super::errors::UpdateError> {
                let parent = path.parent().ok_or_else(|| {
                    super::errors::UpdateError::Parse("missing schedule directory".into())
                })?;
                std::fs::create_dir_all(parent)?;
                let mut file = tempfile::NamedTempFile::new_in(parent)?;
                serde_json::to_writer(file.as_file_mut(), self)
                    .map_err(|e| super::errors::UpdateError::Parse(e.to_string()))?;
                file.persist(path)
                    .map_err(|e| super::errors::UpdateError::Io(e.error))?;
                Ok(())
            })();
            if let Err(error) = result {
                tracing::warn!(%error, "could not persist update schedule");
            }
        }
    }

    fn path() -> Option<std::path::PathBuf> {
        dirs::config_dir().map(|path| path.join("ropy/update-check.json"))
    }

    pub(crate) const fn is_due(&self, now: u64) -> bool {
        now >= self.next_check || now < self.last_check
    }

    pub(crate) const fn channel_changed(&self, prerelease: bool) -> bool {
        self.prerelease != prerelease
    }

    pub(crate) fn record(&mut self, now: u64, success: bool, prerelease: bool) {
        self.last_check = now;
        self.prerelease = prerelease;
        self.failures = if success {
            0
        } else {
            self.failures.saturating_add(1)
        };
        let delay = if success {
            86_400
        } else {
            (900_u64 << self.failures.saturating_sub(1).min(7)).min(86_400)
        };
        self.next_check = now.saturating_add(delay);
    }
}

pub(crate) fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schedule_repeated_failure_caps_delay_at_one_day() {
        let mut schedule = CheckSchedule::default();
        for _ in 0..100 {
            schedule.record(100, false, false);
        }
        assert_eq!(schedule.next_check, 86_500);
    }

    #[test]
    fn test_schedule_success_waits_one_day() {
        let mut schedule = CheckSchedule::default();
        assert!(schedule.is_due(100));
        schedule.record(100, true, false);
        assert!(!schedule.is_due(101));
        assert!(schedule.is_due(100 + 86_400));
    }

    #[test]
    fn test_schedule_failure_backs_off_and_success_resets() {
        let mut schedule = CheckSchedule::default();
        schedule.record(100, false, false);
        assert_eq!(schedule.next_check, 100 + 900);
        schedule.record(1000, false, false);
        assert_eq!(schedule.next_check, 1000 + 1800);
        schedule.record(3000, true, false);
        assert_eq!(schedule.failures, 0);
    }

    #[test]
    fn test_schedule_channel_change_bypasses_previous_cadence() {
        let mut schedule = CheckSchedule::default();
        schedule.record(100, true, false);
        assert!(schedule.channel_changed(true));
        assert!(!schedule.channel_changed(false));
    }
}

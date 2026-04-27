//! Validation helpers applied after `Settings` is deserialized.
//!
//! Each method clamps or resets a field group so that out-of-range values on
//! disk cannot propagate into the running application. Keeping them in a
//! dedicated module keeps `mod.rs` focused on type definitions and I/O.

use std::str::FromStr;

use super::Settings;

impl Settings {
    /// Validate hotkey and reset to default if invalid.
    pub(super) fn validate_hotkey(&mut self) {
        if self.hotkey.activation_key.is_empty()
            || global_hotkey::hotkey::HotKey::from_str(&self.hotkey.activation_key).is_err()
        {
            self.hotkey.activation_key = Self::default().hotkey.activation_key;
        }
    }

    pub(super) fn validate_window_opacity(&mut self) {
        self.window.normalize_opacity();
    }

    pub(super) fn validate_storage(&mut self) {
        self.storage.max_history_records = self.storage.max_history_records.clamp(1, 10_000);
        self.storage.max_storage_records = self.storage.max_storage_records.clamp(1, 100_000);
        if self.storage.max_storage_records < self.storage.max_history_records {
            self.storage.max_storage_records = self.storage.max_history_records;
        }
    }
}

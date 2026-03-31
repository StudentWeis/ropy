//! Synchronization helpers.

use std::sync::{Mutex, MutexGuard};

/// Acquire a mutex guard and recover the inner value if the mutex was poisoned.
pub fn lock_or_recover<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
#[allow(clippy::significant_drop_tightening)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::lock_or_recover;

    #[test]
    fn test_lock_or_recover_returns_guard_for_healthy_mutex() {
        let mutex = Mutex::new(vec![1]);

        let mut guard = lock_or_recover(&mutex);
        guard.push(2);

        assert_eq!(*guard, vec![1, 2]);
    }

    #[test]
    fn test_lock_or_recover_recovers_poisoned_mutex() {
        let mutex = Arc::new(Mutex::new(String::from("before panic")));
        let poisoned_mutex = Arc::clone(&mutex);

        let _ = std::thread::spawn(move || {
            let mut guard = match poisoned_mutex.lock() {
                Ok(guard) => guard,
                Err(err) => panic!("failed to lock mutex before poisoning: {err}"),
            };
            guard.push_str(" during panic");
            panic!("poison mutex for recovery test");
        })
        .join();

        let mut guard = lock_or_recover(&mutex);
        guard.push_str(" after recovery");

        assert_eq!(guard.as_str(), "before panic during panic after recovery");
    }
}

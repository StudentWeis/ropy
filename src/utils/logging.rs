//! Logging initialization utilities.
//!
//! Provides JSON logging with daily rolling files.

use std::path::PathBuf;

use tracing_subscriber::fmt;

use crate::config::Settings;

pub struct LoggingGuard {
    _file_guard: tracing_appender::non_blocking::WorkerGuard,
}

fn default_env_filter() -> tracing_subscriber::EnvFilter {
    tracing_subscriber::EnvFilter::new("info")
}

fn default_log_dir() -> PathBuf {
    let base = Settings::config_dir()
        .ok()
        .or_else(|| dirs::config_dir().map(|dir| dir.join("ropy")))
        .unwrap_or_else(|| std::env::temp_dir().join("ropy"));

    base.join("logs")
}

/// Returns the directory where log files are stored.
pub fn log_dir() -> PathBuf {
    default_log_dir()
}

fn build_file_subscriber(
    writer: impl for<'a> fmt::MakeWriter<'a> + Send + Sync + 'static,
) -> impl tracing::Subscriber + Send + Sync {
    tracing_subscriber::fmt()
        .with_env_filter(default_env_filter())
        .json()
        .flatten_event(true)
        .with_timer(fmt::time::UtcTime::rfc_3339())
        .with_current_span(true)
        .with_span_list(true)
        .with_writer(writer)
        .finish()
}

/// Initialize JSON logging.
///
/// - Enabled by default
/// - Daily rolling file under the user's config directory: `.../ropy/logs/`
/// - Format: JSON lines
///
/// Returns a guard that must be kept alive for the program lifetime, otherwise
/// buffered logs may be dropped.
#[must_use]
pub fn init() -> Option<LoggingGuard> {
    let _ = tracing_log::LogTracer::init();

    let log_dir = default_log_dir();
    if let Err(err) = std::fs::create_dir_all(&log_dir) {
        let subscriber = build_file_subscriber(std::io::stdout);
        if tracing::subscriber::set_global_default(subscriber).is_ok() {
            tracing::warn!(%err, path = %log_dir.display(), "failed to create log directory; falling back to stdout");
        }
        return None;
    }

    let file_appender = match tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(tracing_appender::rolling::Rotation::DAILY)
        .filename_prefix("ropy")
        .filename_suffix("jsonl")
        .max_log_files(30)
        .build(&log_dir)
    {
        Ok(appender) => appender,
        Err(err) => {
            let subscriber = build_file_subscriber(std::io::stdout);
            if tracing::subscriber::set_global_default(subscriber).is_ok() {
                tracing::warn!(%err, path = %log_dir.display(), "failed to create rolling file appender; falling back to stdout");
            }
            return None;
        }
    };
    let (non_blocking, file_guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = build_file_subscriber(non_blocking);

    match tracing::subscriber::set_global_default(subscriber) {
        Ok(()) => Some(LoggingGuard {
            _file_guard: file_guard,
        }),
        Err(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_subscriber_does_not_panic() {
        let subscriber = build_file_subscriber(std::io::sink);
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(answer = 42, "test log");
        });
    }

    #[test]
    fn test_default_log_dir_is_stable() {
        let dir = default_log_dir();
        assert!(dir.ends_with("logs"));
    }
}

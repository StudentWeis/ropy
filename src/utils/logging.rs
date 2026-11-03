//! Logging initialization utilities.
//!
//! Provides JSON logging with daily rolling files.

use std::path::PathBuf;

use tracing_subscriber::fmt;

use crate::config::Settings;

pub(crate) struct LoggingGuard {
    _file_guard: tracing_appender::non_blocking::WorkerGuard,
}

fn default_log_dir() -> PathBuf {
    let base = Settings::config_dir()
        .ok()
        .or_else(|| dirs::config_dir().map(|dir| dir.join("ropy")))
        .unwrap_or_else(|| std::env::temp_dir().join("ropy"));

    base.join("logs")
}

/// Returns the directory where log files are stored.
pub(crate) fn log_dir() -> PathBuf {
    default_log_dir()
}

fn build_file_subscriber(
    writer: impl for<'a> fmt::MakeWriter<'a> + Send + Sync + 'static,
) -> impl tracing::Subscriber + Send + Sync {
    fmt()
        .with_max_level(tracing::Level::INFO)
        .json()
        .flatten_event(true)
        .with_timer(fmt::time::UtcTime::rfc_3339())
        .with_current_span(true)
        .with_span_list(true)
        .with_writer(writer)
        .finish()
}

/// Capture startup and update-helper diagnostics without accessing user log files.
pub(crate) fn with_stderr_logging<R>(operation: impl FnOnce() -> R) -> R {
    tracing::subscriber::with_default(build_file_subscriber(std::io::stderr), operation)
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
pub(crate) fn init() -> Option<LoggingGuard> {
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
    #[expect(clippy::unwrap_used, reason = "test I/O and JSON output must be valid")]
    fn test_file_subscriber_filters_and_preserves_json_context() {
        use std::io::{Read, Seek};

        let mut output = tempfile::tempfile().unwrap();
        let writer = output.try_clone().unwrap();
        let subscriber = build_file_subscriber(move || writer.try_clone().unwrap());
        tracing::subscriber::with_default(subscriber, || {
            let span = tracing::info_span!("operation", request_id = 7);
            let _entered = span.enter();
            tracing::trace!("hidden trace");
            tracing::debug!("hidden debug");
            tracing::info!(answer = 42, "test log");
            tracing::warn!("test warning");
            tracing::error!("test error");
        });

        output.rewind().unwrap();
        let mut json = String::new();
        output.read_to_string(&mut json).unwrap();
        let events: Vec<serde_json::Value> = json
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(events.len(), 3);
        for (event, level) in events.iter().zip(["INFO", "WARN", "ERROR"]) {
            assert_eq!(event["level"], level);
            let timestamp = event["timestamp"].as_str().unwrap();
            assert!(chrono::DateTime::parse_from_rfc3339(timestamp).is_ok());
            assert_eq!(event["span"]["name"], "operation");
            assert_eq!(event["span"]["request_id"], 7);
            assert_eq!(event["spans"][0], event["span"]);
        }
        assert_eq!(events[0]["message"], "test log");
        assert_eq!(events[0]["answer"], 42);
    }

    #[test]
    fn test_default_log_dir_is_stable() {
        let dir = default_log_dir();
        assert!(dir.ends_with("logs"));
    }
}

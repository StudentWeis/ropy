//! Shared curl command builder – eliminates duplicated HTTP request logic
//! across the updater module.

use std::process::Command;

use super::errors::UpdateError;

pub const CONNECT_TIMEOUT_SECS: u32 = 15;
pub const API_REQUEST_MAX_TIME_SECS: u32 = 30;
pub const DOWNLOAD_REQUEST_MAX_TIME_SECS: u32 = 600;

/// Builder for constructing `curl` subprocess commands with consistent
/// base arguments (silent, follow redirects, fail on HTTP errors, User-Agent).
pub struct CurlCommandBuilder {
    command: Command,
}

impl CurlCommandBuilder {
    /// Create a new builder with the common base flags: `-sSL --fail` and the
    /// `User-Agent` header.
    pub fn new(url: &str) -> Self {
        let mut command = Command::new("curl");
        command.args([
            "-sSL",
            "--fail",
            "-H",
            &format!("User-Agent: ropy/{}", env!("CARGO_PKG_VERSION")),
        ]);
        command.arg(url);
        Self { command }
    }

    /// Add an extra HTTP header (e.g. `Accept: application/vnd.github.v3+json`).
    pub fn header(mut self, value: &str) -> Self {
        self.command.args(["-H", value]);
        self
    }

    /// Set the `--connect-timeout` value in seconds.
    pub fn connect_timeout(mut self, seconds: u32) -> Self {
        self.command
            .args(["--connect-timeout", &seconds.to_string()]);
        self
    }

    /// Set the `--max-time` value in seconds.
    pub fn max_time(mut self, seconds: u32) -> Self {
        self.command.args(["--max-time", &seconds.to_string()]);
        self
    }

    /// Apply the standard timeout policy for short-lived API requests.
    pub fn with_api_timeouts(self) -> Self {
        self.connect_timeout(CONNECT_TIMEOUT_SECS)
            .max_time(API_REQUEST_MAX_TIME_SECS)
    }

    /// Apply the standard timeout policy for large asset downloads.
    pub fn with_download_timeouts(self) -> Self {
        self.connect_timeout(CONNECT_TIMEOUT_SECS)
            .max_time(DOWNLOAD_REQUEST_MAX_TIME_SECS)
    }

    /// Execute the command, collect stdout, and return the body as a `String`.
    ///
    /// Logs and returns an `UpdateError::Network` on failure.
    pub fn execute_to_string(mut self) -> Result<String, UpdateError> {
        let output = self.command.output().map_err(|e| {
            tracing::error!(error = %e, "failed to launch curl");
            UpdateError::Network(format!("failed to launch curl: {e}"))
        })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(status = %output.status, stderr = %stderr, "curl request failed");
            return Err(UpdateError::Network(format!(
                "HTTP request failed (exit {}): {stderr}",
                output.status
            )));
        }

        String::from_utf8(output.stdout).map_err(|e| {
            tracing::error!(error = %e, "response body is not valid UTF-8");
            UpdateError::Network(format!("invalid UTF-8 in response: {e}"))
        })
    }

    /// Return the inner `Command` for callers that need piped I/O (e.g.
    /// streaming downloads with progress tracking).
    pub fn into_command(self) -> Command {
        self.command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command_args(command: &Command) -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn test_with_api_timeouts_appends_expected_args() {
        let command = CurlCommandBuilder::new("https://example.com")
            .with_api_timeouts()
            .into_command();

        let args = command_args(&command);
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--connect-timeout", "15"])
        );
        assert!(args.windows(2).any(|pair| pair == ["--max-time", "30"]));
    }

    #[test]
    fn test_with_download_timeouts_appends_expected_args() {
        let command = CurlCommandBuilder::new("https://example.com/archive.tar.xz")
            .with_download_timeouts()
            .into_command();

        let args = command_args(&command);
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--connect-timeout", "15"])
        );
        assert!(args.windows(2).any(|pair| pair == ["--max-time", "600"]));
    }
}

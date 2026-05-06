//! Shared `curl` command builder. Centralises the base flags (silent,
//! follow redirects, fail on HTTP errors, `User-Agent`) so individual call
//! sites can't drift apart on retry / timeout policy.

use std::process::Command;

use super::errors::UpdateError;

pub const CONNECT_TIMEOUT_SECS: u32 = 15;
pub const API_REQUEST_MAX_TIME_SECS: u32 = 30;
pub const DOWNLOAD_REQUEST_MAX_TIME_SECS: u32 = 600;

pub struct CurlCommandBuilder {
    command: Command,
}

impl CurlCommandBuilder {
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

    pub fn header(mut self, value: &str) -> Self {
        self.command.args(["-H", value]);
        self
    }

    pub fn connect_timeout(mut self, seconds: u32) -> Self {
        self.command
            .args(["--connect-timeout", &seconds.to_string()]);
        self
    }

    pub fn max_time(mut self, seconds: u32) -> Self {
        self.command.args(["--max-time", &seconds.to_string()]);
        self
    }

    /// Standard policy for short-lived JSON API calls (release listings).
    pub fn with_api_timeouts(self) -> Self {
        self.connect_timeout(CONNECT_TIMEOUT_SECS)
            .max_time(API_REQUEST_MAX_TIME_SECS)
    }

    /// Standard policy for large asset downloads — much longer max-time
    /// because release archives can run into hundreds of MB.
    pub fn with_download_timeouts(self) -> Self {
        self.connect_timeout(CONNECT_TIMEOUT_SECS)
            .max_time(DOWNLOAD_REQUEST_MAX_TIME_SECS)
    }

    /// Run to completion and decode stdout as UTF-8. Failures (non-zero
    /// exit, non-UTF-8 body) are mapped to [`UpdateError::Network`] and
    /// logged, since every callsite would otherwise repeat the same code.
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

    /// Escape hatch for callers that need piped stdio (streaming downloads
    /// with progress tracking) and therefore can't go through
    /// [`Self::execute_to_string`].
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

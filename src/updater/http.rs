//! Shared `curl` command builder.
//!
//! Keeps update HTTP flags consistent across release checks and downloads.

use std::process::Command;

use super::errors::UpdateError;

pub(crate) const CONNECT_TIMEOUT_SECS: u32 = 15;
pub(crate) const API_REQUEST_MAX_TIME_SECS: u32 = 30;
pub(crate) const DOWNLOAD_REQUEST_MAX_TIME_SECS: u32 = 600;
const HTTP_STATUS_MARKER: &str = "__ROPY_HTTP_STATUS__:";
const HTTP_STATUS_WRITE_OUT: &str = "__ROPY_HTTP_STATUS__:%{http_code}";

pub(crate) struct CurlCommandBuilder {
    command: Command,
}

impl CurlCommandBuilder {
    pub(crate) fn new(url: &str) -> Self {
        let mut command = Command::new("curl");
        command.args([
            "-sSL",
            "-H",
            &format!("User-Agent: ropy/{}", env!("CARGO_PKG_VERSION")),
        ]);
        command.arg(url);
        Self { command }
    }

    pub(crate) fn header(mut self, value: &str) -> Self {
        self.command.args(["-H", value]);
        self
    }

    pub(crate) fn connect_timeout(mut self, seconds: u32) -> Self {
        self.command
            .args(["--connect-timeout", &seconds.to_string()]);
        self
    }

    pub(crate) fn max_time(mut self, seconds: u32) -> Self {
        self.command.args(["--max-time", &seconds.to_string()]);
        self
    }

    /// Standard policy for short-lived JSON API calls (release listings).
    pub(crate) fn with_api_timeouts(self) -> Self {
        self.connect_timeout(CONNECT_TIMEOUT_SECS)
            .max_time(API_REQUEST_MAX_TIME_SECS)
    }

    /// Standard policy for large asset downloads — much longer max-time
    /// because release archives can run into hundreds of MB.
    pub(crate) fn with_download_timeouts(self) -> Self {
        self.connect_timeout(CONNECT_TIMEOUT_SECS)
            .max_time(DOWNLOAD_REQUEST_MAX_TIME_SECS)
    }

    /// Run to completion, capture the final HTTP status, and decode stdout as
    /// UTF-8. HTTP failures retain their response body so callers can
    /// distinguish service rate limits from connection failures.
    pub(crate) fn execute_to_string(mut self) -> Result<String, UpdateError> {
        self.command.args(["--write-out", HTTP_STATUS_WRITE_OUT]);
        let output = self.command.output().map_err(|e| {
            tracing::error!(error = %e, "failed to launch curl");
            UpdateError::Network(format!("failed to launch curl: {e}"))
        })?;

        let stdout = String::from_utf8(output.stdout).map_err(|e| {
            tracing::error!(error = %e, "response body is not valid UTF-8");
            UpdateError::Network(format!("invalid UTF-8 in response: {e}"))
        })?;

        if !output.status.success() {
            let body = stdout
                .rsplit_once(HTTP_STATUS_MARKER)
                .map_or(stdout.as_str(), |(body, _)| body);
            let stderr = String::from_utf8_lossy(&output.stderr);
            tracing::error!(status = %output.status, stderr = %stderr, body = %body, "curl request failed");
            return Err(curl_failure(&output.status.to_string(), body, &stderr));
        }

        let (body, http_status) = parse_http_response(&stdout)?;
        if !(200..300).contains(&http_status) {
            tracing::error!(status = http_status, body = %body, "HTTP request failed");
            return Err(curl_failure(&format!("HTTP {http_status}"), body, ""));
        }

        Ok(body.to_string())
    }

    /// Escape hatch for callers that need piped stdio (streaming downloads
    /// with progress tracking) and therefore can't go through
    /// [`Self::execute_to_string`].
    pub(crate) fn into_command(mut self) -> Command {
        self.command.arg("--fail");
        self.command
    }
}

fn parse_http_response(response: &str) -> Result<(&str, u16), UpdateError> {
    let (body, status) = response.rsplit_once(HTTP_STATUS_MARKER).ok_or_else(|| {
        UpdateError::Network("curl response did not include an HTTP status".into())
    })?;
    let status = status
        .parse()
        .map_err(|e| UpdateError::Network(format!("invalid HTTP status '{status}': {e}")))?;
    Ok((body, status))
}

fn curl_failure(status: &str, body: &str, stderr: &str) -> UpdateError {
    if body.to_ascii_lowercase().contains("rate limit exceeded") {
        return UpdateError::RateLimited;
    }

    let body = body.trim();
    let detail = if body.is_empty() {
        stderr.trim().to_string()
    } else {
        body.chars().take(512).collect()
    };

    UpdateError::Network(format!("HTTP request failed ({status}): {detail}"))
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

    #[test]
    fn test_curl_failure_with_github_rate_limit_body_returns_rate_limited_error() {
        let error = curl_failure(
            "exit status: 22",
            r#"{"message":"API rate limit exceeded for 192.0.2.1."}"#,
            "curl: (22) The requested URL returned error: 403",
        );

        assert!(matches!(error, UpdateError::RateLimited));
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_parse_http_response_separates_body_and_status() {
        let (body, status) =
            parse_http_response(r#"{"tag_name":"0.6.0"}__ROPY_HTTP_STATUS__:200"#).unwrap();

        assert_eq!(body, r#"{"tag_name":"0.6.0"}"#);
        assert_eq!(status, 200);
    }
}

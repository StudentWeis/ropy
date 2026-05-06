//! Download, verify, extract, and replace the running binary.

use std::{
    io::Read,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

use super::{errors::UpdateError, models::ReleaseInfo};

/// Download the release asset, verify its checksum, extract the binary, and
/// replace the running executable.
///
/// `progress_tx` is an `async_channel` sender that reports download progress
/// (values between 0.0 and 1.0) to the UI thread.
pub fn download_and_install(
    release: &ReleaseInfo,
    progress_tx: &async_channel::Sender<f32>,
) -> Result<(), UpdateError> {
    let tmp_dir = tempfile::tempdir().map_err(UpdateError::Io)?;
    let asset_name = release
        .download_url
        .rsplit('/')
        .next()
        .unwrap_or("ropy-update");
    let asset_path = tmp_dir.path().join(asset_name);

    // 1. Download the archive
    tracing::info!(url = %release.download_url, dest = %asset_path.display(), size = release.asset_size, "downloading update asset");
    download_file(
        &release.download_url,
        &asset_path,
        release.asset_size,
        progress_tx,
    )?;

    // 2. Verify checksum (if available)
    if release.checksum_url.is_empty() {
        tracing::warn!("no checksum URL provided – skipping verification");
    } else {
        tracing::info!("verifying checksum");
        verify_checksum(&asset_path, &release.checksum_url)?;
    }

    // 3. Extract the binary from the archive
    tracing::info!("extracting binary from archive");
    let binary_path = extract_binary(&asset_path, tmp_dir.path())?;

    // 4. Replace the running executable
    tracing::info!("replacing running executable");
    self_replace::self_replace(&binary_path).map_err(|e| UpdateError::Replace(e.to_string()))?;

    // 5. On Unix set the executable permission (self_replace should handle this, but be safe)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755));
        }
    }

    tracing::info!("update installed successfully – restart required");
    Ok(())
}

/// Download `url` into `dest`, reporting progress via `progress_tx`.
///
/// Uses `curl` subprocess with piped stdout to stream the download and track
/// progress, avoiding macOS firewall restrictions on raw sockets.
fn download_file(
    url: &str,
    dest: &Path,
    total_size: u64,
    progress_tx: &async_channel::Sender<f32>,
) -> Result<(), UpdateError> {
    use std::process::Stdio;

    let mut curl_command = super::http::CurlCommandBuilder::new(url)
        .with_download_timeouts()
        .into_command();

    let mut child = curl_command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            tracing::error!(url = %url, error = %e, "failed to launch curl");
            UpdateError::Network(format!("failed to launch curl: {e}"))
        })?;

    let mut reader = child
        .stdout
        .take()
        .ok_or_else(|| UpdateError::Network("failed to capture curl stdout".into()))?;

    let mut file = std::fs::File::create(dest).map_err(UpdateError::Io)?;

    let mut downloaded: u64 = 0;
    let mut buf = [0u8; 8192];
    loop {
        let n = reader.read(&mut buf).map_err(|e| {
            tracing::error!(url = %url, error = %e, "failed reading curl output");
            UpdateError::Io(e)
        })?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut file, &buf[..n]).map_err(UpdateError::Io)?;
        downloaded += n as u64;
        if total_size > 0 {
            #[allow(
                clippy::cast_precision_loss,
                reason = "download progress only needs sub-percent accuracy"
            )]
            let progress = downloaded as f32 / total_size as f32;
            let _ = progress_tx.send_blocking(progress);
        }
    }

    let status = child.wait().map_err(|e| {
        tracing::error!(error = %e, "failed to wait for curl");
        UpdateError::Io(e)
    })?;

    if !status.success() {
        return Err(UpdateError::Network(format!(
            "curl download failed (exit {status})"
        )));
    }

    let _ = progress_tx.send_blocking(1.0);
    Ok(())
}

/// Download the `.sha256` file and verify the asset against it.
fn verify_checksum(asset_path: &Path, checksum_url: &str) -> Result<(), UpdateError> {
    let checksum_body = super::http::CurlCommandBuilder::new(checksum_url)
        .with_api_timeouts()
        .execute_to_string()?;

    // The checksum file format from cargo-dist is:  "<hex>  <filename>\n"
    let expected_hex = checksum_body
        .split_whitespace()
        .next()
        .unwrap_or("")
        .to_lowercase();

    let actual_hex = compute_sha256_hex(asset_path)?;

    if actual_hex != expected_hex {
        return Err(UpdateError::ChecksumMismatch {
            expected: expected_hex,
            actual: actual_hex,
        });
    }
    Ok(())
}

fn compute_sha256_hex(asset_path: &Path) -> Result<String, UpdateError> {
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(asset_path)?;
    let mut buffer = [0_u8; 8 * 1024];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Extract the `ropy` (or `ropy.exe`) binary from the archive and return its
/// path inside `out_dir`.
fn extract_binary(archive_path: &Path, out_dir: &Path) -> Result<PathBuf, UpdateError> {
    let name = archive_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();

    if name.ends_with(".tar.xz") {
        extract_tar_xz(archive_path, out_dir)
    } else if name.ends_with(".zip") {
        extract_zip(archive_path, out_dir)
    } else {
        Err(UpdateError::Extract(format!(
            "unsupported archive format: {name}"
        )))
    }
}

/// Extract a `.tar.xz` archive (macOS / Linux)
#[cfg(not(target_os = "windows"))]
fn extract_tar_xz(archive_path: &Path, out_dir: &Path) -> Result<PathBuf, UpdateError> {
    let file = std::fs::File::open(archive_path)?;
    let decompressor = xz2::read::XzDecoder::new(file);
    let mut archive = tar::Archive::new(decompressor);

    archive
        .unpack(out_dir)
        .map_err(|e| UpdateError::Extract(e.to_string()))?;

    find_binary_in_dir(out_dir)
}

/// Stub for Windows – `.tar.xz` is not expected on this platform.
#[cfg(target_os = "windows")]
fn extract_tar_xz(_archive_path: &Path, _out_dir: &Path) -> Result<PathBuf, UpdateError> {
    Err(UpdateError::Extract(
        "tar.xz extraction is not supported on Windows".into(),
    ))
}

/// Extract a `.zip` archive (Windows)
#[cfg(target_os = "windows")]
fn extract_zip(archive_path: &Path, out_dir: &Path) -> Result<PathBuf, UpdateError> {
    let file = std::fs::File::open(archive_path)?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| UpdateError::Extract(e.to_string()))?;
    archive
        .extract(out_dir)
        .map_err(|e| UpdateError::Extract(e.to_string()))?;

    find_binary_in_dir(out_dir)
}

/// Stub for non-Windows – `.zip` is not expected on this platform.
#[cfg(not(target_os = "windows"))]
fn extract_zip(_archive_path: &Path, _out_dir: &Path) -> Result<PathBuf, UpdateError> {
    Err(UpdateError::Extract(
        "zip extraction is not supported on this platform".into(),
    ))
}

/// Walk `dir` and find the first file named `ropy` or `ropy.exe`.
fn find_binary_in_dir(dir: &Path) -> Result<PathBuf, UpdateError> {
    for entry in walkdir(dir)? {
        let name = entry.file_name().unwrap_or_default().to_string_lossy();
        if name == "ropy" || name == "ropy.exe" {
            return Ok(entry);
        }
    }
    Err(UpdateError::Extract(
        "could not find ropy binary in extracted archive".into(),
    ))
}

/// Simple recursive directory walk (avoids adding the `walkdir` crate).
fn walkdir(dir: &Path) -> Result<Vec<PathBuf>, UpdateError> {
    let mut result = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            result.extend(walkdir(&path)?);
        } else {
            result.push(path);
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_verify_checksum_format_parsing() {
        // Simulate a cargo-dist style checksum line
        let line = "abcdef1234567890  ropy-aarch64-apple-darwin.tar.xz\n";
        let expected = line.split_whitespace().next().unwrap().to_lowercase();
        assert_eq!(expected, "abcdef1234567890");
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_find_binary_in_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let bin_path = tmp.path().join("ropy");
        std::fs::write(&bin_path, b"fake").unwrap();

        let found = find_binary_in_dir(tmp.path()).unwrap();
        assert_eq!(found, bin_path);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_find_binary_in_nested_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let nested = tmp.path().join("ropy-aarch64-apple-darwin");
        std::fs::create_dir_all(&nested).unwrap();
        let bin_path = nested.join("ropy");
        std::fs::write(&bin_path, b"fake").unwrap();

        let found = find_binary_in_dir(tmp.path()).unwrap();
        assert_eq!(found, bin_path);
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_find_binary_missing() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("readme.txt"), b"hi").unwrap();

        let result = find_binary_in_dir(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    fn test_compute_sha256_hex_reads_file_content_expected() {
        let tmp = tempfile::tempdir().unwrap();
        let asset_path = tmp.path().join("asset.bin");
        std::fs::write(&asset_path, b"ropy checksum fixture").unwrap();

        let actual = compute_sha256_hex(&asset_path).unwrap();

        assert_eq!(
            actual,
            "e480cb6fb62b8a6be76827b9e23059e7e82730289a575088f2ce14956806b865"
        );
    }

    #[test]
    #[allow(clippy::unwrap_used)]
    #[allow(clippy::float_cmp)]
    fn test_progress_channel_send_blocking_delivers_values() {
        let (sender, receiver) = async_channel::unbounded::<f32>();

        sender.send_blocking(0.25).unwrap();
        sender.send_blocking(0.5).unwrap();
        sender.send_blocking(1.0).unwrap();

        assert_eq!(receiver.try_recv().unwrap(), 0.25);
        assert_eq!(receiver.try_recv().unwrap(), 0.5);
        assert_eq!(receiver.try_recv().unwrap(), 1.0);
    }

    #[test]
    fn test_progress_channel_drop_sender_closes_receiver() {
        let (sender, receiver) = async_channel::unbounded::<f32>();

        drop(sender);

        assert!(receiver.try_recv().is_err());
        assert!(receiver.is_closed());
    }
}

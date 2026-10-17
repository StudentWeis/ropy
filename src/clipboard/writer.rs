use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
use image::ImageReader;

use super::{ClipboardWriteError, ClipboardWriteResult, CopyRequest};

/// Run the single consumer that owns the OS [`ClipboardContext`].
/// All copy operations funnel through the supplied channel so we don't pay
/// the cost of recreating the context (and re-acquiring platform handles)
/// on every write.
pub(crate) async fn write_clipboard(rx: async_channel::Receiver<CopyRequest>) {
    let ctx = match ClipboardContext::new() {
        Ok(ctx) => ctx,
        Err(e) => {
            tracing::error!(error = %e, "failed to create clipboard output context");
            return;
        }
    };
    while let Ok(req) = rx.recv().await {
        match req {
            CopyRequest::Text { text, completion } => {
                notify_completion(completion, set_text(&ctx, text));
            }
            CopyRequest::Image { path, completion } => {
                notify_completion(completion, set_image(&ctx, &path));
            }
            CopyRequest::Files { paths, completion } => {
                notify_completion(completion, set_files(&ctx, &paths));
            }
            CopyRequest::RichText {
                plain_text,
                html,
                rtf,
                completion,
            } => {
                notify_completion(completion, set_rich_text(&ctx, plain_text, html, rtf));
            }
        }
    }
}

fn load_image_from_path(path: &str) -> image::ImageResult<image::DynamicImage> {
    ImageReader::open(path)
        .map_err(image::ImageError::from)
        .and_then(ImageReader::decode)
}

fn clipboard_error(error: impl std::fmt::Display) -> ClipboardWriteError {
    ClipboardWriteError::Clipboard(error.to_string())
}

fn set_text(ctx: &ClipboardContext, text: String) -> ClipboardWriteResult {
    ctx.set_text(text).map_err(clipboard_error)
}

fn load_and_set_image(
    path: &str,
    set_image: impl FnOnce(image::DynamicImage) -> ClipboardWriteResult,
) -> ClipboardWriteResult {
    let image = load_image_from_path(path)?;
    set_image(image)
}

fn set_image(ctx: &ClipboardContext, path: &str) -> ClipboardWriteResult {
    load_and_set_image(path, |img| {
        #[cfg(target_os = "macos")]
        {
            // `clipboard-rs::set_image` on macOS clears the pasteboard
            // before encoding, so for large images the listener can fire
            // on the empty intermediate state. Pre-encode here and use
            // `set_buffer` to keep clear→write atomic from its POV.
            let mut bytes = Vec::new();
            img.write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )?;
            ctx.set_buffer("public.png", bytes).map_err(clipboard_error)
        }

        #[cfg(not(target_os = "macos"))]
        {
            use clipboard_rs::common::RustImage;

            let rust_image = clipboard_rs::RustImageData::from_dynamic_image(img);
            ctx.set_image(rust_image).map_err(clipboard_error)
        }
    })
}

fn set_files(ctx: &ClipboardContext, paths: &[String]) -> ClipboardWriteResult {
    write_files_with(
        paths,
        |contents| ctx.set(contents).map_err(clipboard_error),
        |files| ctx.set_files(files).map_err(clipboard_error),
    )
}

fn write_files_with(
    paths: &[String],
    set_contents: impl FnOnce(Vec<ClipboardContent>) -> ClipboardWriteResult,
    set_file_list: impl FnOnce(Vec<String>) -> ClipboardWriteResult,
) -> ClipboardWriteResult {
    if paths.is_empty() {
        return Err(ClipboardWriteError::EmptyFileList);
    }

    #[cfg(target_os = "linux")]
    let clipboard_paths: Vec<_> = paths.iter().map(|path| file_uri_from_path(path)).collect();
    #[cfg(not(target_os = "linux"))]
    let clipboard_paths = paths.to_vec();
    let contents = vec![
        ClipboardContent::Text(paths.join("\n")),
        ClipboardContent::Files(clipboard_paths.clone()),
    ];

    if set_contents(contents).is_ok() {
        return Ok(());
    }
    set_file_list(clipboard_paths)
}

#[cfg(target_os = "linux")]
fn file_uri_from_path(path: &str) -> String {
    // clipboard-rs forwards file URIs unchanged on X11 and Wayland, but only
    // prefixes bare paths. Escape filename bytes before crossing that boundary.
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut uri = String::from("file://");
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            uri.push('%');
            uri.push(char::from(HEX[usize::from(byte >> 4)]));
            uri.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    uri
}

fn set_rich_text(
    ctx: &ClipboardContext,
    plain_text: String,
    html: Option<String>,
    rtf: Option<String>,
) -> ClipboardWriteResult {
    let mut contents = vec![ClipboardContent::Text(plain_text.clone())];
    if let Some(html_content) = html {
        contents.push(ClipboardContent::Html(html_content));
    }
    if let Some(rtf_content) = rtf {
        contents.push(ClipboardContent::Rtf(rtf_content));
    }

    if ctx.set(contents).is_ok() {
        return Ok(());
    }
    ctx.set_text(plain_text).map_err(clipboard_error)
}

fn notify_completion(
    completion: Option<async_channel::Sender<ClipboardWriteResult>>,
    result: ClipboardWriteResult,
) {
    if let Err(error) = &result {
        tracing::warn!(error = %error, "failed to write clipboard content");
    }
    if let Some(tx) = completion {
        let _ = tx.try_send(result);
    }
}

#[cfg(test)]
mod tests {
    use std::assert_matches;

    use super::*;

    #[cfg(target_os = "linux")]
    #[rstest::rstest]
    #[case("/tmp/report%20final.txt", "file:///tmp/report%2520final.txt")]
    #[case("/tmp/report final.txt", "file:///tmp/report%20final.txt")]
    #[case("/tmp/report#1?.txt", "file:///tmp/report%231%3F.txt")]
    #[case("/tmp/report\r\nnext.txt", "file:///tmp/report%0D%0Anext.txt")]
    #[case("/tmp/中文.txt", "file:///tmp/%E4%B8%AD%E6%96%87.txt")]
    #[case("/tmp/report-1_~.txt", "file:///tmp/report-1_~.txt")]
    fn test_file_uri_from_path_preserves_filename_on_recapture(
        #[case] path: &str,
        #[case] expected_uri: &str,
    ) {
        let uri = file_uri_from_path(path);

        assert_eq!(uri, expected_uri);
        assert_eq!(crate::repository::normalize_file_paths(&[uri]), vec![path]);
    }

    #[cfg(target_os = "linux")]
    #[rstest::rstest]
    #[case(false)]
    #[case(true)]
    fn test_write_files_with_encodes_both_outputs_and_preserves_plain_text(
        #[case] fail_combined_write: bool,
    ) {
        let paths = vec![
            "/tmp/report%20#\n中文.txt".into(),
            "/tmp/second file.txt".into(),
        ];
        let expected = vec![
            "file:///tmp/report%2520%23%0A%E4%B8%AD%E6%96%87.txt".to_string(),
            "file:///tmp/second%20file.txt".to_string(),
        ];
        let fallback_called = std::cell::Cell::new(false);

        let result = write_files_with(
            &paths,
            |contents| {
                assert!(
                    matches!(&contents[0], ClipboardContent::Text(text) if text == &paths.join("\n"))
                );
                assert!(
                    matches!(&contents[1], ClipboardContent::Files(files) if files == &expected)
                );
                if fail_combined_write {
                    Err(ClipboardWriteError::Clipboard("injected failure".into()))
                } else {
                    Ok(())
                }
            },
            |files| {
                fallback_called.set(true);
                assert_eq!(files, expected);
                Ok(())
            },
        );

        assert!(result.is_ok(), "{result:?}");
        assert_eq!(fallback_called.get(), fail_combined_write);
    }

    #[test]
    #[expect(clippy::unwrap_used)]
    fn test_load_image_from_path_when_png_exists_returns_decoded_image() {
        let temp_dir = tempfile::tempdir().unwrap();
        let image_path = temp_dir.path().join("test.png");
        image::DynamicImage::new_rgba8(2, 3)
            .save(&image_path)
            .unwrap();

        let image = load_image_from_path(image_path.to_str().unwrap()).unwrap();

        assert_eq!(image.width(), 2);
        assert_eq!(image.height(), 3);
    }

    #[test]
    fn test_load_image_from_path_when_file_is_missing_returns_error() {
        let result = load_image_from_path("/definitely/missing/image.png");

        assert!(result.is_err());
    }

    #[test]
    fn test_set_image_when_file_is_missing_returns_error() {
        let setter_called = std::cell::Cell::new(false);
        let result = load_and_set_image("/definitely/missing/image.png", |_| {
            setter_called.set(true);
            Ok(())
        });

        assert_matches!(result, Err(ClipboardWriteError::Image(_)));
        assert!(!setter_called.get());
    }

    #[test]
    fn test_notify_completion_when_sender_exists_sends_result() {
        let (completion_tx, completion_rx) = async_channel::bounded(1);

        notify_completion(Some(completion_tx), Err(ClipboardWriteError::EmptyFileList));

        assert_matches!(
            completion_rx.try_recv(),
            Ok(Err(ClipboardWriteError::EmptyFileList))
        );
    }

    #[test]
    fn test_notify_completion_when_sender_missing_returns_without_panic() {
        notify_completion(None, Ok(()));
    }
}

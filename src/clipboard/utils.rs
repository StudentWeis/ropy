#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]
use std::{
    fs,
    path::{Path, PathBuf},
};

use image::{DynamicImage, GenericImageView};
use thiserror::Error;

use crate::repository::RichTextMeta;

const THUMBNAIL_MAX_DIMENSION: u32 = 180;

fn create_thumbnail(image: &DynamicImage) -> DynamicImage {
    let (width, height) = image.dimensions();
    if width <= THUMBNAIL_MAX_DIMENSION && height <= THUMBNAIL_MAX_DIMENSION {
        return image.clone();
    }

    image.thumbnail(THUMBNAIL_MAX_DIMENSION, THUMBNAIL_MAX_DIMENSION)
}

pub(super) fn image_path_for_hash(images_dir: &Path, image_content_hash: u64) -> PathBuf {
    images_dir.join(format!("{image_content_hash}.png"))
}

pub(crate) fn thumb_path_for(original: &Path) -> PathBuf {
    let stem = original.file_stem().unwrap_or_default().to_string_lossy();

    original.extension().map_or_else(
        || original.with_file_name(format!("{stem}_thumb")),
        |extension| {
            let extension = extension.to_string_lossy();
            original.with_file_name(format!("{stem}_thumb.{extension}"))
        },
    )
}

#[derive(Debug, Error)]
pub(crate) enum ImageSaveError {
    #[error("application data directory unavailable")]
    DataDirNotFound,
    #[error("failed to write image cache: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to encode image cache: {0}")]
    Encode(#[from] image::ImageError),
}

fn write_image_atomically(
    path: &Path,
    encode: impl FnOnce(&mut fs::File) -> Result<(), ImageSaveError>,
) -> Result<(), ImageSaveError> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("image path has no parent"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    encode(temporary.as_file_mut())?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

fn save_png(image: &DynamicImage, path: &Path) -> Result<(), ImageSaveError> {
    write_image_atomically(path, |file| {
        image.write_to(file, image::ImageFormat::Png)?;
        Ok(())
    })
}

fn save_image_to_dir(
    image: &DynamicImage,
    image_content_hash: u64,
    data_dir: &Path,
) -> Result<PathBuf, ImageSaveError> {
    fs::create_dir_all(data_dir)?;
    let file_path = image_path_for_hash(data_dir, image_content_hash);
    let thumb_file_path = thumb_path_for(&file_path);

    // Decode the complete PNG: existence or a readable header cannot certify
    // a previous write completed. A fresh capture can repair either asset.
    if image::open(&file_path).is_err() {
        save_png(image, &file_path)?;
    }
    if image::open(&thumb_file_path).is_err() {
        save_png(&create_thumbnail(image), &thumb_file_path)?;
    }
    Ok(file_path)
}

fn rich_text_dir_path(data_dir: &Path) -> PathBuf {
    data_dir.join("rich_text")
}

fn write_rich_text_file(
    data_dir: &Path,
    record_id: u64,
    extension: &str,
    content: &str,
) -> Option<String> {
    use std::io::Write;
    let directory = rich_text_dir_path(data_dir);
    fs::create_dir_all(&directory).ok()?;
    let mut file = tempfile::Builder::new()
        .prefix(&format!("{record_id}-"))
        .suffix(&format!(".{extension}"))
        .tempfile_in(directory)
        .ok()?;
    file.write_all(content.as_bytes()).ok()?;
    file.as_file().sync_all().ok()?;
    let (_, path) = file.keep().ok()?;
    Some(path.to_string_lossy().into_owned())
}

pub(crate) fn save_rich_text_files_to_dir(
    record_id: u64,
    html: Option<&str>,
    rtf: Option<&str>,
    data_dir: &Path,
) -> Option<RichTextMeta> {
    let html_path =
        html.and_then(|content| write_rich_text_file(data_dir, record_id, "html", content));
    let rtf_path =
        rtf.and_then(|content| write_rich_text_file(data_dir, record_id, "rtf", content));

    if html_path.is_none() && rtf_path.is_none() {
        None
    } else {
        Some(RichTextMeta {
            html_path,
            rtf_path,
        })
    }
}

pub(crate) fn save_image(
    image: &DynamicImage,
    image_content_hash: u64,
) -> Result<String, ImageSaveError> {
    let data_dir = dirs::data_local_dir()
        .ok_or(ImageSaveError::DataDirNotFound)?
        .join("ropy")
        .join("images");

    save_image_to_dir(image, image_content_hash, &data_dir)
        .map(|file_path| file_path.to_string_lossy().to_string())
}

pub(crate) fn load_rich_text_html(meta: &RichTextMeta) -> Option<String> {
    meta.html_path
        .as_deref()
        .and_then(|path| fs::read_to_string(path).ok())
}

pub(crate) fn load_rich_text_rtf(meta: &RichTextMeta) -> Option<String> {
    meta.rtf_path
        .as_deref()
        .and_then(|path| fs::read_to_string(path).ok())
}

pub(crate) fn remove_rich_text_files(meta: &RichTextMeta) {
    if let Some(path) = meta.html_path.as_deref() {
        let _ = fs::remove_file(path);
    }
    if let Some(path) = meta.rtf_path.as_deref() {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        path::Path,
        thread,
        time::{Duration, SystemTime},
    };

    use image::{DynamicImage, GenericImageView};
    use tempfile::tempdir;

    use super::{
        THUMBNAIL_MAX_DIMENSION, create_thumbnail, image_path_for_hash, load_rich_text_html,
        load_rich_text_rtf, remove_rich_text_files, save_image_to_dir, save_rich_text_files_to_dir,
        thumb_path_for,
    };

    #[rstest::rstest]
    #[case(b"")]
    #[case(b"\x89PNG\r\n\x1a\n")]
    fn test_save_image_to_dir_corrupt_cache_repairs_pixels(#[case] corrupt: &[u8]) {
        let dir = tempdir().expect("fixture directory");
        let image = DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            20,
            10,
            image::Rgba([12, 34, 56, 255]),
        ));
        let path = image_path_for_hash(dir.path(), 42);
        std::fs::write(&path, corrupt).expect("corrupt fixture");
        std::fs::write(thumb_path_for(&path), corrupt).expect("corrupt thumbnail");

        let saved = save_image_to_dir(&image, 42, dir.path()).expect("repair cache");

        assert_eq!(
            image::open(saved).expect("decode original").to_rgba8(),
            image.to_rgba8()
        );
        assert_eq!(
            image::open(thumb_path_for(&path))
                .expect("decode thumbnail")
                .to_rgba8(),
            image.to_rgba8()
        );
    }

    #[rstest::rstest]
    #[case(false)]
    #[case(true)]
    fn test_write_image_atomically_failed_encoding_preserves_destination(#[case] existing: bool) {
        use std::io::Write as _;
        let dir = tempdir().expect("fixture directory");
        let path = dir.path().join("42.png");
        let image = DynamicImage::new_rgba8(10, 10);
        if existing {
            super::save_png(&image, &path).expect("initial image");
        }
        let before = std::fs::read(&path).ok();
        let result = super::write_image_atomically(&path, |file| {
            file.write_all(b"partial PNG")?;
            Err(std::io::Error::other("injected write failure").into())
        });
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).ok(), before);
        assert_eq!(
            std::fs::read_dir(dir.path())
                .expect("list fixtures")
                .count(),
            usize::from(existing)
        );
    }

    #[test]
    fn test_create_thumbnail_scales_large_image_within_limit() {
        let image = DynamicImage::new_rgba8(400, 100);

        let thumbnail = create_thumbnail(&image);

        assert_eq!(thumbnail.dimensions(), (THUMBNAIL_MAX_DIMENSION, 45));
    }

    #[test]
    fn test_create_thumbnail_keeps_small_image_size() {
        let image = DynamicImage::new_rgba8(90, 60);

        let thumbnail = create_thumbnail(&image);

        assert_eq!(thumbnail.dimensions(), (90, 60));
    }

    #[test]
    fn test_image_path_for_hash_uses_content_hash_as_file_name() {
        let path = image_path_for_hash(Path::new("/tmp/ropy/images"), 42);

        assert_eq!(path, Path::new("/tmp/ropy/images/42.png"));
    }

    #[test]
    fn test_thumb_path_for_appends_suffix_before_extension() {
        let thumb_path = thumb_path_for(Path::new("/tmp/ropy/images/42.png"));

        assert_eq!(thumb_path, Path::new("/tmp/ropy/images/42_thumb.png"));
    }

    #[test]
    fn test_save_image_to_dir_when_image_exists_skips_rewrite() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let image = DynamicImage::new_rgba8(400, 100);
        let hash = 42_u64;

        let file_path = save_image_to_dir(&image, hash, temp_dir.path())
            .expect("Failed to save image the first time");
        let thumb_path = thumb_path_for(&file_path);
        let original_modified = std::fs::metadata(&file_path)
            .expect("Failed to read image metadata")
            .modified()
            .expect("Failed to read image modification time");
        let original_thumb_modified = std::fs::metadata(&thumb_path)
            .expect("Failed to read thumbnail metadata")
            .modified()
            .expect("Failed to read thumbnail modification time");

        thread::sleep(Duration::from_millis(20));

        let saved_path = save_image_to_dir(&image, hash, temp_dir.path())
            .expect("Failed to save image the second time");
        let image_modified = std::fs::metadata(&file_path)
            .expect("Failed to re-read image metadata")
            .modified()
            .expect("Failed to re-read image modification time");
        let thumb_modified = std::fs::metadata(&thumb_path)
            .expect("Failed to re-read thumbnail metadata")
            .modified()
            .expect("Failed to re-read thumbnail modification time");

        assert_eq!(saved_path, file_path);
        assert_eq!(image_modified, original_modified);
        assert_eq!(thumb_modified, original_thumb_modified);
    }

    #[test]
    fn test_save_image_to_dir_when_thumbnail_missing_recreates_thumbnail() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let image = DynamicImage::new_rgba8(400, 100);
        let hash = 7_u64;

        let file_path = save_image_to_dir(&image, hash, temp_dir.path())
            .expect("Failed to save image the first time");
        let thumb_path = thumb_path_for(&file_path);
        std::fs::remove_file(&thumb_path).expect("Failed to remove thumbnail");

        thread::sleep(Duration::from_millis(20));

        save_image_to_dir(&image, hash, temp_dir.path())
            .expect("Failed to save image after deleting thumbnail");
        let thumb_modified = std::fs::metadata(&thumb_path)
            .expect("Failed to read recreated thumbnail metadata")
            .modified()
            .expect("Failed to read recreated thumbnail modification time");

        assert!(thumb_modified <= SystemTime::now());
    }

    #[test]
    fn test_save_rich_text_files_to_dir_writes_present_payloads() {
        let temp_dir = tempdir().expect("Failed to create temp dir");

        let meta = save_rich_text_files_to_dir(
            42,
            Some("<p>hello</p>"),
            Some("{\\rtf1 hello}"),
            temp_dir.path(),
        )
        .expect("Expected rich text metadata");

        assert_eq!(load_rich_text_html(&meta).as_deref(), Some("<p>hello</p>"));
        assert_eq!(load_rich_text_rtf(&meta).as_deref(), Some("{\\rtf1 hello}"));
    }

    #[test]
    fn test_remove_rich_text_files_deletes_saved_sidecars() {
        let temp_dir = tempdir().expect("Failed to create temp dir");

        let meta = save_rich_text_files_to_dir(
            7,
            Some("<p>hello</p>"),
            Some("{\\rtf1 hello}"),
            temp_dir.path(),
        )
        .expect("Expected rich text metadata");
        let html_path = meta.html_path.clone().expect("Expected html path");
        let rtf_path = meta.rtf_path.clone().expect("Expected rtf path");

        remove_rich_text_files(&meta);

        assert!(!Path::new(&html_path).exists());
        assert!(!Path::new(&rtf_path).exists());
    }
}

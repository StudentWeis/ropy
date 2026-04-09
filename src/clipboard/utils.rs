use std::{
    fs,
    path::{Path, PathBuf},
};

use image::{DynamicImage, GenericImageView};

use crate::repository::RichTextMeta;

const THUMBNAIL_MAX_DIMENSION: u32 = 180;

fn create_thumbnail(image: &DynamicImage) -> DynamicImage {
    let (width, height) = image.dimensions();
    if width <= THUMBNAIL_MAX_DIMENSION && height <= THUMBNAIL_MAX_DIMENSION {
        return image.clone();
    }

    image.thumbnail(THUMBNAIL_MAX_DIMENSION, THUMBNAIL_MAX_DIMENSION)
}

pub fn image_path_for_hash(images_dir: &Path, image_content_hash: u64) -> PathBuf {
    images_dir.join(format!("{image_content_hash}.png"))
}

pub fn thumb_path_for(original: &Path) -> PathBuf {
    let stem = original.file_stem().unwrap_or_default().to_string_lossy();

    original.extension().map_or_else(
        || original.with_file_name(format!("{stem}_thumb")),
        |extension| {
            let extension = extension.to_string_lossy();
            original.with_file_name(format!("{stem}_thumb.{extension}"))
        },
    )
}

fn save_image_to_dir(
    image: &DynamicImage,
    image_content_hash: u64,
    data_dir: &Path,
) -> Option<PathBuf> {
    if !data_dir.exists() {
        std::fs::create_dir_all(data_dir).ok()?;
    }

    let file_path = image_path_for_hash(data_dir, image_content_hash);
    let thumb_file_path = thumb_path_for(&file_path);
    let image_exists = file_path.exists();

    if !image_exists {
        image
            .save_with_format(&file_path, image::ImageFormat::Png)
            .ok()?;
    }

    if !image_exists || !thumb_file_path.exists() {
        let thumb = create_thumbnail(image);
        thumb
            .save_with_format(&thumb_file_path, image::ImageFormat::Png)
            .ok()?;
    }

    Some(file_path)
}

fn rich_text_dir_path(data_dir: &Path) -> PathBuf {
    data_dir.join("rich_text")
}

fn rich_text_file_path(data_dir: &Path, record_id: u64, extension: &str) -> PathBuf {
    rich_text_dir_path(data_dir).join(format!("{record_id}.{extension}"))
}

fn write_rich_text_file(path: &Path, content: &str) -> Option<String> {
    let parent = path.parent()?;
    if !parent.exists() {
        fs::create_dir_all(parent).ok()?;
    }

    fs::write(path, content).ok()?;
    Some(path.to_string_lossy().to_string())
}

pub fn save_rich_text_files_to_dir(
    record_id: u64,
    html: Option<&str>,
    rtf: Option<&str>,
    data_dir: &Path,
) -> Option<RichTextMeta> {
    let html_path = html.and_then(|content| {
        let path = rich_text_file_path(data_dir, record_id, "html");
        write_rich_text_file(&path, content)
    });
    let rtf_path = rtf.and_then(|content| {
        let path = rich_text_file_path(data_dir, record_id, "rtf");
        write_rich_text_file(&path, content)
    });

    if html_path.is_none() && rtf_path.is_none() {
        None
    } else {
        Some(RichTextMeta {
            html_path,
            rtf_path,
        })
    }
}

pub fn save_image(image: &DynamicImage, image_content_hash: u64) -> Option<String> {
    let data_dir = dirs::data_local_dir()?.join("ropy").join("images");

    save_image_to_dir(image, image_content_hash, &data_dir)
        .map(|file_path| file_path.to_string_lossy().to_string())
}

pub fn load_rich_text_html(meta: &RichTextMeta) -> Option<String> {
    meta.html_path
        .as_deref()
        .and_then(|path| fs::read_to_string(path).ok())
}

pub fn load_rich_text_rtf(meta: &RichTextMeta) -> Option<String> {
    meta.rtf_path
        .as_deref()
        .and_then(|path| fs::read_to_string(path).ok())
}

pub fn remove_rich_text_files(meta: &RichTextMeta) {
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
    #[allow(clippy::expect_used)]
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
    #[allow(clippy::expect_used)]
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
    #[allow(clippy::expect_used)]
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
    #[allow(clippy::expect_used)]
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

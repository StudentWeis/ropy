use chrono::Local;
use image::{DynamicImage, GenericImageView};

const THUMBNAIL_MAX_DIMENSION: u32 = 180;

fn create_thumbnail(image: &DynamicImage) -> DynamicImage {
    let (width, height) = image.dimensions();
    if width <= THUMBNAIL_MAX_DIMENSION && height <= THUMBNAIL_MAX_DIMENSION {
        return image.clone();
    }

    image.thumbnail(THUMBNAIL_MAX_DIMENSION, THUMBNAIL_MAX_DIMENSION)
}

pub fn save_image(image: &DynamicImage) -> Option<String> {
    let data_dir = dirs::data_local_dir()?.join("ropy").join("images");
    if !data_dir.exists() {
        std::fs::create_dir_all(&data_dir).ok()?;
    }

    let now = Local::now();
    let id = now.timestamp_nanos_opt().unwrap_or(0) as u64;
    let file_name = format!("{id}.png");
    let file_path = data_dir.join(&file_name);

    image
        .save_with_format(&file_path, image::ImageFormat::Png)
        .ok()?;

    // Save thumbnail
    let thumb_file_name = format!("{id}_thumb.png");
    let thumb_file_path = data_dir.join(&thumb_file_name);
    let thumb = create_thumbnail(image);
    thumb
        .save_with_format(&thumb_file_path, image::ImageFormat::Png)
        .ok()?;

    Some(file_path.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, GenericImageView};

    use super::{THUMBNAIL_MAX_DIMENSION, create_thumbnail};

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
}

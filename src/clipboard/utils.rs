use chrono::Local;
use image::DynamicImage;

pub fn save_image(image: DynamicImage) -> Option<String> {
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
    let thumb = image.thumbnail(300, 300);
    thumb
        .save_with_format(&thumb_file_path, image::ImageFormat::Png)
        .ok()?;

    Some(file_path.to_string_lossy().to_string())
}

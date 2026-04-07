fn main() {
    // Inject the build target triple for auto-update platform detection
    println!(
        "cargo:rustc-env=TARGET={}",
        std::env::var("TARGET").unwrap_or_default()
    );

    #[cfg(target_os = "windows")]
    {
        let out_dir = match std::env::var("OUT_DIR") {
            Ok(path) => path,
            Err(e) => {
                println!("cargo:warning=OUT_DIR is not set: {e}");
                return;
            }
        };
        let icon_path = std::path::Path::new(&out_dir).join("logo.ico");

        // Convert png to ico if it doesn't exist or if png is newer.
        println!("cargo:rerun-if-changed=assets/logo.png");

        if let Ok(img) = image::open("assets/logo.png") {
            let img = if img.width() > 256 || img.height() > 256 {
                img.resize(256, 256, image::imageops::FilterType::Lanczos3)
            } else {
                img
            };

            if let Err(e) = img.save_with_format(&icon_path, image::ImageFormat::Ico) {
                println!("cargo:warning=Failed to save logo.ico: {e}");
            } else {
                let mut res = winres::WindowsResource::new();
                if let Some(icon_path_str) = icon_path.to_str() {
                    res.set_icon(icon_path_str);
                } else {
                    println!(
                        "cargo:warning=Icon path is not valid UTF-8: {}",
                        icon_path.display()
                    );
                    return;
                }
                if let Err(e) = res.compile() {
                    println!("cargo:warning=Failed to compile Windows resource: {e}");
                }
            }
        } else {
            println!("cargo:warning=assets/logo.png not found");
        }
    }
}

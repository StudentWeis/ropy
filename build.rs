use std::env;
use std::path::Path;

fn main() {
    #[cfg(target_os = "windows")]
    {
        let out_dir = env::var("OUT_DIR").unwrap();
        let icon_path = Path::new(&out_dir).join("logo.ico");
        
        // Convert png to ico if it doesn't exist or if png is newer.
        println!("cargo:rerun-if-changed=assets/logo.png");

        if let Ok(img) = image::open("assets/logo.png") {
             let img = if img.width() > 256 || img.height() > 256 {
                 img.resize(256, 256, image::imageops::FilterType::Lanczos3)
             } else {
                 img
             };

             if let Err(e) = img.save_with_format(&icon_path, image::ImageFormat::Ico) {
                 println!("cargo:warning=Failed to save logo.ico: {}", e);
             } else {
                 let mut res = winres::WindowsResource::new();
                 res.set_icon(icon_path.to_str().unwrap());
                 if let Err(e) = res.compile() {
                     println!("cargo:warning=Failed to compile Windows resource: {}", e);
                 }
             }
        } else {
             println!("cargo:warning=assets/logo.png not found");
        }
    }
}

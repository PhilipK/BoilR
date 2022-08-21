pub mod ui_colors {
    use egui::Color32;

    pub const TEXT_COLOR: Color32 = Color32::from_rgb(255, 212, 163);
    pub const EXTRA_BACKGROUND_COLOR: Color32 = Color32::from_rgb(10, 30, 60);
    pub const BACKGROUND_COLOR: Color32 = Color32::from_rgb(13, 43, 69);
    pub const BG_STROKE_COLOR: Color32 = Color32::from_rgb(32, 60, 86);
    pub const LIGHT_ORANGE: Color32 = Color32::from_rgb(255, 212, 163);
    pub const ORANGE: Color32 = Color32::from_rgb(255, 170, 94);
    pub const PURLPLE: Color32 = Color32::from_rgb(84, 78, 104);
}

pub mod ui_images {
    use std::{
        path::Path,
        thread::{self, Thread},
    };

    use eframe::IconData;
    use egui::{ColorImage, ImageData};

    pub const IMPORT_GAMES_IMAGE: &[u8] = include_bytes!("../../resources/import_games_button.png");
    pub const SAVE_IMAGE: &[u8] = include_bytes!("../../resources/save.png");
    pub const LOGO_32: &[u8] = include_bytes!("../../resources/logo32.png");
    pub const LOGO_ICON: &[u8] = include_bytes!("../../resources/logo_small.png");

    pub fn get_import_image() -> ImageData {
        ImageData::Color(load_image_from_memory(IMPORT_GAMES_IMAGE).unwrap())
    }

    pub fn get_save_image() -> ImageData {
        ImageData::Color(load_image_from_memory(SAVE_IMAGE).unwrap())
    }

    pub fn get_logo() -> ImageData {
        ImageData::Color(load_image_from_memory(LOGO_32).unwrap())
    }

    pub fn get_logo_icon() -> IconData {
        let image = image::load_from_memory(LOGO_ICON).unwrap();
        let image_buffer = image.to_rgba8();
        let pixels = image_buffer.as_flat_samples();
        IconData {
            height: image.height() as u32,
            width: image.width() as u32,
            rgba: pixels.as_slice().to_vec(),
        }
    }
    pub fn load_image_from_path(path: &Path) -> Option<ColorImage> {
        if path.exists() {
            if let Ok(data) = std::fs::read(path) {
                let load_result = load_image_from_memory(&data);
                if load_result.is_err() {
                    eprintln!("Could not load image at path {:?}", path);
                }
                return load_result.ok();
            }
        }
        None
    }

    fn load_image_from_memory(image_data: &[u8]) -> Result<ColorImage, image::ImageError> {
        let image = image::load_from_memory(image_data)?;
        let size = [image.width() as _, image.height() as _];
        let image_buffer = image.to_rgba8();
        let pixels = image_buffer.as_flat_samples();
        thread::scope(|s| {
            let rgba = pixels.as_slice();
            let is_valid = size[0] * size[1] * 4 == rgba.len();
            if is_valid {
                //Wrapping this in a thread, since it has a tendency to panic
                let thread_handle = s
                    .spawn(move || ColorImage::from_rgba_unmultiplied(size, rgba))
                    .join();
                match thread_handle {
                    Ok(value) => Ok(value),
                    Err(e) => {
                        println!("Error loading image {:?}", e);
                        Err(image::ImageError::Decoding(
                            image::error::DecodingError::new(
                                image::error::ImageFormatHint::Unknown,
                                "Could not load image, it panicked while trying",
                            ),
                        ))
                    }
                }
            } else {
                Err(image::ImageError::Decoding(
                    image::error::DecodingError::new(
                        image::error::ImageFormatHint::Unknown,
                        "Image did not have right amount of pixels",
                    ),
                ))
            }
        })
    }

}

#[cfg(test)]
mod tests {
    use super::ui_images::load_image_from_path;

    #[test]
    pub fn test_image_load_that_is_broken() {
        let res = load_image_from_path(std::path::Path::new("src/testdata/brokenimage.webp"));
        assert!(res.is_none());
    }

    #[test]
    pub fn test_image_load_that_works_png() {
        let res = load_image_from_path(std::path::Path::new("src/testdata/smallpng.png"));
        assert!(res.is_some());
    }

    #[test]
    pub fn test_image_load_that_works_webp() {
        let res = load_image_from_path(std::path::Path::new("src/testdata/spider.webp"));
        assert!(res.is_some());
    }
}

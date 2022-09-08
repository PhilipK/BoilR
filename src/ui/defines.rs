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

   pub fn load_image_from_path(path: &std::path::Path) -> Result<egui::ColorImage, image::ImageError> {
        let image = image::io::Reader::open(path)?.with_guessed_format()?.decode()?;
        let size = [image.width() as _, image.height() as _];
        let image_buffer = image.to_rgba8();
        let pixels = image_buffer.as_flat_samples();
        Ok(egui::ColorImage::from_rgba_unmultiplied(
            size,
            pixels.as_slice(),
        ))
    }
    
   pub fn load_image_from_memory(image_data: &[u8]) -> Result<ColorImage, image::ImageError> {
        let image = image::load_from_memory(image_data)?;
        let size = [image.width() as _, image.height() as _];
        let image_buffer = image.to_rgba8();
        let pixels = image_buffer.as_flat_samples();
        Ok(ColorImage::from_rgba_unmultiplied(
            size,
            pixels.as_slice(),
        ))
    }

}

#[cfg(test)]
mod tests {
    use super::ui_images::load_image_from_path;

    #[test]
    pub fn test_image_load_that_is_broken() {
        let res = load_image_from_path(std::path::Path::new("src/testdata/brokenimage.webp"));
        assert!(res.is_err());
    }

    #[test]
    pub fn test_image_load_that_works_png() {
        let res = load_image_from_path(std::path::Path::new("src/testdata/smallpng.png"));
        assert!(res.is_ok());
    }

    #[test]
    pub fn test_image_load_that_works_webp() {
        let res = load_image_from_path(std::path::Path::new("src/testdata/spider.webp"));
        assert!(res.is_ok());
    }

    #[test]
    pub fn test_image_load_animated_webp() {
        let res = load_image_from_path(std::path::Path::new("src/testdata/hollow.webp"));
        assert!(res.is_err());
    }

    #[test]
    pub fn test_image_load_animated_webp2() {
        let res = load_image_from_path(std::path::Path::new("src/testdata/tunic.webp"));
        assert!(res.is_err());
    }
}

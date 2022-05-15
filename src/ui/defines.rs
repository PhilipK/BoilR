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
    use std::path::Path;

    use eframe::IconData;
    use egui::{ColorImage, ImageData};

    pub const IMPORT_GAMES_IMAGE: &[u8] = include_bytes!("../../resources/import_games_button.png");
    pub const LOGO_32: &[u8] = include_bytes!("../../resources/logo32.png");
    pub const LOGO_ICON: &[u8] = include_bytes!("../../resources/logo_small.png");

    pub fn get_import_image() -> ImageData {
        ImageData::Color(load_image_from_memory(IMPORT_GAMES_IMAGE).unwrap())
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
                return load_image_from_memory(&data).ok();
            }
        }
        None
    }

    fn load_image_from_memory(image_data: &[u8]) -> Result<ColorImage, image::ImageError> {
        let image = image::load_from_memory(image_data)?;
        let size = [image.width() as _, image.height() as _];
        let image_buffer = image.to_rgba8();
        let pixels = image_buffer.as_flat_samples();

        Ok(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()))
    }
}

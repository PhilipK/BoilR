pub mod ui_images{
    use egui::{ImageData, ColorImage};

    pub const IMPORT_GAMES_IMAGE: &[u8] = include_bytes!("../../resources/import_games_button.png");
    


    pub fn get_import_image() -> ImageData {
        ImageData::Color(load_image_from_memory(IMPORT_GAMES_IMAGE).unwrap())
    }


    fn load_image_from_memory(image_data: &[u8]) -> Result<ColorImage, image::ImageError> {
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
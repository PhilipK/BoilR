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


    use egui::IconData;

    pub const LOGO_ICON: &[u8] = include_bytes!("../../resources/logo_small.png");

    pub fn get_logo_icon() -> IconData {
        let image = image::load_from_memory(LOGO_ICON).unwrap_or_default();
        let image_buffer = image.to_rgba8();
        let pixels = image_buffer.as_flat_samples();
        IconData {
            height: image.height(),
            width: image.width(),
            rgba: pixels.as_slice().to_vec(),
        }
    }

}
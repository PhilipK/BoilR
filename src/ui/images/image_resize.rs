pub fn clamp_to_width(size: &mut egui::Vec2, max_width: f32) {
    let mut x = size.x;
    let mut y = size.y;
    if size.x > max_width {
        let ratio = size.y / size.x;
        x = max_width;
        y = x * ratio;
    }
    size.x = x;
    size.y = y;
}
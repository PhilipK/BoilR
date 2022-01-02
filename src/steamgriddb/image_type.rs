#[derive(Debug, Clone, Copy)]
pub enum ImageType {
    Hero,
    Grid,
    Logo,
    BigPicture,
}

impl ImageType {
    pub fn file_name(&self, app_id: u32) -> String {
        match self {
            ImageType::Hero => format!("{}_hero.png", app_id),
            ImageType::Grid => format!("{}p.png", app_id),
            ImageType::Logo => format!("{}_logo.png", app_id),
            ImageType::BigPicture => format!("{}_bigpicture.png", app_id),
        }
    }
}

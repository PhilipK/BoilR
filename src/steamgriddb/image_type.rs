#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ImageType {
    Hero,
    Grid,
    WideGrid,
    Logo,
    BigPicture,
    Icon,
}

pub const ALL_TYPES: [ImageType; 6] = [
    ImageType::Hero,
    ImageType::Grid,
    ImageType::WideGrid,
    ImageType::Logo,
    ImageType::BigPicture,
    ImageType::Icon,
];

impl ImageType {
    pub fn all() -> &'static [ImageType; 6] {
        &ALL_TYPES
    }

    pub fn name(&self) -> &str {
        match self {
            ImageType::Hero => "Hero",
            ImageType::Grid => "Grid",
            ImageType::WideGrid => "Wide Grid",
            ImageType::Logo => "Logo",
            ImageType::BigPicture => "Big Picture",
            ImageType::Icon => "Icon",
        }
    }

    pub fn file_name<S: AsRef<str>>(&self, app_id: u32, extension: S) -> String {
        let file_name = self.file_name_no_extension(app_id);
        format!("{}.{}", file_name, extension.as_ref())
    }

    pub fn file_name_no_extension(&self, app_id: u32) -> String {
        match self {
            ImageType::Hero => format!("{}_hero", app_id),
            ImageType::Grid => format!("{}p", app_id),
            ImageType::WideGrid => format!("{}", app_id),
            ImageType::Logo => format!("{}_logo", app_id),
            ImageType::BigPicture => format!("{}_bigpicture", app_id),
            ImageType::Icon => format!("{}-icon", app_id),
        }
    }

    pub fn steam_url<S: AsRef<str>>(&self, steam_app_id: S, mtime: u64) -> String {
        let steam_app_id = steam_app_id.as_ref();
        match self {
            ImageType::Hero => format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_hero.jpg?t={}",
                steam_app_id, mtime
            ),
            ImageType::Grid => format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900_2x.jpg?t={}",
                steam_app_id, mtime
            ),
            ImageType::WideGrid => format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg?t={}",
                steam_app_id, mtime
            ),
            ImageType::Logo => format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/logo.png?t={}",
                steam_app_id, mtime
            ),
            ImageType::BigPicture => format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg?t={}",
                steam_app_id, mtime
            ),
            // This should not happen
            _ => "".to_string(),
        }
    }
}

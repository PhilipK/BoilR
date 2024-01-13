#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            ImageType::Hero => format!("{app_id}_hero"),
            ImageType::Grid => format!("{app_id}p"),
            ImageType::WideGrid => format!("{app_id}"),
            ImageType::Logo => format!("{app_id}_logo"),
            ImageType::BigPicture => format!("{app_id}_bigpicture"),
            ImageType::Icon => format!("{app_id}-icon"),
        }
    }

    pub fn steam_url<S: AsRef<str>>(&self, steam_app_id: S, mtime: u64) -> String {
        let steam_app_id = steam_app_id.as_ref();
        match self {
            ImageType::Hero => format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{steam_app_id}/library_hero.jpg?t={mtime}"
            ),
            ImageType::Grid => format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{steam_app_id}/library_600x900_2x.jpg?t={mtime}"
            ),
            ImageType::WideGrid => format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{steam_app_id}/header.jpg?t={mtime}"
            ),
            ImageType::Logo => format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{steam_app_id}/logo.png?t={mtime}"
            ),
            ImageType::BigPicture => format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{steam_app_id}/header.jpg?t={mtime}"
            ),
            // This should not happen
            _ => "".to_string(),
        }
    }
}

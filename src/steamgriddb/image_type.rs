#[derive(Debug, Clone, Copy)]
pub enum ImageType {
    Hero,
    Grid,
    WideGrid,
    Logo,
    BigPicture,
}

impl ImageType {
    pub fn file_name(&self, app_id: u32) -> String {
        match self {
            ImageType::Hero => format!("{}_hero.png", app_id),
            ImageType::Grid => format!("{}p.png", app_id),
            ImageType::WideGrid => format!("{}.png", app_id),
            ImageType::Logo => format!("{}_logo.png", app_id),
            ImageType::BigPicture => format!("{}_bigpicture.png", app_id),
        }
    }

    pub fn steam_url<S:AsRef<str>>(&self,steam_app_id:S,mtime:u64) -> String{
        let steam_app_id = steam_app_id.as_ref();
        match self{
            ImageType::Hero => format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_hero.jpg?t={}",steam_app_id,mtime),
            ImageType::Grid => format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/library_600x900_2x.jpg?t={}",steam_app_id,mtime),
            ImageType::WideGrid => format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg?t={}",steam_app_id,mtime),
            ImageType::Logo => format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/logo.png?t={}",steam_app_id,mtime),
            ImageType::BigPicture => format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{}/header.jpg?t={}",steam_app_id,mtime),
        }
    }
}

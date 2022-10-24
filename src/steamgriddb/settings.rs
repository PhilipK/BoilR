use serde::{Deserialize, Serialize};

use super::ImageType;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SteamGridDbSettings {
    pub enabled: bool,
    pub auth_key: Option<String>,
    pub prefer_animated: bool,
    pub banned_images: Vec<String>,
    pub only_download_boilr_images: bool,
    pub allow_nsfw: bool,
}

impl SteamGridDbSettings {
    pub fn is_image_banned(&self, image_type: &ImageType, app_id: u32) -> bool {
        let ban_id = format!("{}-{}", app_id, image_type.name());
        self.banned_images.contains(&ban_id)
    }

    pub fn set_image_banned(&mut self, image_type: &ImageType, app_id: u32, should_ban: bool) {
        let ban_id = format!("{}-{}", app_id, image_type.name());
        let images_banned = &mut self.banned_images;
        let is_banned = images_banned.contains(&ban_id);
        match (is_banned, should_ban) {
            (true, false) => images_banned.retain(|i| !i.eq(&ban_id)),
            (false, true) => images_banned.push(ban_id),
            _ => {}
        }
    }
}

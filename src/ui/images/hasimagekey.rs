use std::path::{Path, PathBuf};

use steam_shortcuts_util::shortcut::ShortcutOwned;

use crate::{steamgriddb::ImageType, steam::SteamGameInfo};

use super::{gametype::GameType, constants::POSSIBLE_EXTENSIONS};


pub trait HasImageKey {

    ///Gives a unique key to an image given its type and user path
    fn key(&self, image_type: &ImageType, user_path: &Path) -> (PathBuf, String);

}


impl HasImageKey for GameType {
    fn key(&self, image_type: &ImageType, user_path: &Path) -> (PathBuf, String) {
        match self {
            GameType::Shortcut(s) => s.key(image_type, user_path),
            GameType::SteamGame(g) => g.key(image_type, user_path),
        }
    }
}


impl HasImageKey for SteamGameInfo {
    fn key(&self, image_type: &ImageType, user_path: &Path) -> (PathBuf, String) {
        let mut keys = POSSIBLE_EXTENSIONS
            .iter()
            .map(|ext| key_from_extension(self.appid, image_type, user_path, ext));
        let first = keys.next().unwrap();
        let other = keys.find(|(exsists, _, _)| *exsists);
        let (_, path, key) = other.unwrap_or(first);
        (path, key)
    }
}

impl HasImageKey for ShortcutOwned {
    fn key(&self, image_type: &ImageType, user_path: &Path) -> (PathBuf, String) {
        let mut keys = POSSIBLE_EXTENSIONS
            .iter()
            .map(|ext| key_from_extension(self.app_id, image_type, user_path, ext));
        let first = keys.next().unwrap();
        let other = keys.find(|(exsists, _, _)| *exsists);
        let (_, path, key) = other.unwrap_or(first);
        (path, key)
    }
}

fn key_from_extension(
    app_id: u32,
    image_type: &ImageType,
    user_path: &Path,
    ext: &str,
) -> (bool, PathBuf, String) {
    let file_name = image_type.file_name(app_id, ext);
    let path = user_path.join("config").join("grid").join(&file_name);
    let key = path.to_string_lossy().to_string();
    (path.exists(), path, key)
}

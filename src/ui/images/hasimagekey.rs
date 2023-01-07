use std::path::{Path, PathBuf};

use steam_shortcuts_util::shortcut::ShortcutOwned;

use crate::{steam::SteamGameInfo, steamgriddb::ImageType};

use super::{constants::POSSIBLE_EXTENSIONS, gametype::GameType};

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
        let app_id = self.appid;
        key(app_id, image_type, user_path)
    }
}

impl HasImageKey for ShortcutOwned {
    fn key(&self, image_type: &ImageType, user_path: &Path) -> (PathBuf, String) {
        let app_id = self.app_id;
        key(app_id, image_type, user_path)
    }
}

fn key(app_id: u32, image_type: &ImageType, user_path: &Path) -> (PathBuf, String) {
    let ext = |ext| key_from_extension(app_id, image_type, user_path, ext);
    let keys = POSSIBLE_EXTENSIONS.map(ext);
    let other = keys.iter().find(|(exsists, _, _)| *exsists);
    let first = ext(POSSIBLE_EXTENSIONS[0]);
    let (_, path, key) = other.unwrap_or(&first);
    (path.to_path_buf(), key.to_string())
}

fn key_from_extension(
    app_id: u32,
    image_type: &ImageType,
    user_path: &Path,
    ext: &str,
) -> (bool, PathBuf, String) {
    let file_name = image_type.file_name(app_id, ext);
    let path = user_path.join("config").join("grid").join(file_name);
    let key = path.to_string_lossy().to_string();
    (path.exists(), path, key)
}

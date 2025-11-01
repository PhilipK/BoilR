use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use super::{get_steam_path, SteamSettings};

#[derive(Debug, Clone)]
pub struct SteamGameInfo {
    pub appid: u32,
    pub name: String,
}

pub fn get_installed_games(settings: &SteamSettings) -> Vec<SteamGameInfo> {
    let install_folders = get_install_folders(settings);
    let mut games = vec![];
    for apps_path in install_folders {
        if let Ok(files) = std::fs::read_dir(apps_path) {
            for file in files.flatten() {
                if let Some(game_info) = parse_manifest_file(&file.path()) {
                    games.push(game_info);
                }
            }
        }
    }
    games.sort_by_key(|g| g.name.clone());
    games
}

fn get_install_folders(settings: &SteamSettings) -> Vec<PathBuf> {
    let mut result = vec![];
    if let Ok(path) = get_steam_path(settings) {
        let path = Path::new(&path);

        let vdf_path = path.join("steamapps").join("libraryfolders.vdf");
        if !vdf_path.exists() {
            result.push(path.join("steamapps"));
            return result;
        }
        if let Ok(vdf_file) = std::fs::read_to_string(vdf_path) {
            for line in vdf_file.lines() {
                if line.contains("\"path\"") {
                    if let Some(path_string) = line.get(11..line.len() - 1) {
                        result.push(Path::new(&path_string).join("steamapps").to_path_buf());
                    }
                }
            }
        }
    }

    result
}

fn parse_manifest_file(path: &Path) -> Option<SteamGameInfo> {
    let extension = path.extension().and_then(OsStr::to_str);
    if let Some("acf") = extension {
        let file_content = std::fs::read_to_string(path);
        if let Ok(file_content) = file_content {
            return parse_manifest_string(file_content);
        }
    }
    None
}

fn parse_manifest_string<S: AsRef<str>>(string: S) -> Option<SteamGameInfo> {
    let mut lines = string.as_ref().lines();
    let appid: Option<u32> = lines
        .find(|l| l.contains("\"appid\""))
        .and_then(|line| line.get(11..line.len() - 1))
        .and_then(|app_id_str| app_id_str.parse().ok());
    let name_line = lines
        .find(|l| l.contains("\"name\""))
        .and_then(|line| line.get(10..line.len() - 1));
    match (appid, name_line) {
        (Some(appid), Some(name)) => Some(SteamGameInfo {
            name: name.to_string(),
            appid,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    //Okay to unwrap in tests
    #![allow(clippy::unwrap_in_result)]
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn parse_steam_game() {
        let string = include_str!("../testdata/acf/appmanifest_763890.acf");
        let game_info = parse_manifest_string(string);
        assert!(game_info.is_some());
        let game_info = game_info.unwrap();
        assert_eq!("Wildermyth", game_info.name);
        assert_eq!(763890, game_info.appid);
    }

    // #[test]
    // fn installed_files() {
    //     let settings = SteamSettings::default();
    //     let installed_games = get_installed_games(&settings);
    //     assert_eq!(installed_games.len(), 7);
    // }
}

use std::path::Path;

use super::{get_steam_path, SteamSettings};

pub struct SteamGameInfo {
    appid: String,
    name: String,
}

pub fn get_installed_games(settings: &SteamSettings) -> Vec<SteamGameInfo> {
    let path = get_steam_path(settings);
    if let Ok(path) = path {
        let apps_path = Path::new(&path).join("steamapps");
        if let Ok(files) = std::fs::read_dir(apps_path) {
            let mut games = vec![];
            for file in files {
                if let Ok(file) = file {
                    if let Some(game_info) = parse_manifest_file(&file.path()) {
                        games.push(game_info);
                    }
                }
            }
            return games;
        }
    }
    vec![]
}

fn parse_manifest_file(path: &Path) -> Option<SteamGameInfo> {
    if !path.ends_with(".acf") {
        return None;
    }
    let file_content = std::fs::read_to_string(path);
    if let Ok(file_content) = file_content {
        return parse_manifest_string(file_content);
    }

    None
}

fn parse_manifest_string<S: AsRef<str>>(string: S) -> Option<SteamGameInfo> {
    let mut lines = string.as_ref().lines();
    let app_id_line = lines.find(|l| l.contains("\"appid\""));
    let name_line = lines.find(|l| l.contains("\"name\""));
    match (app_id_line, name_line) {
        (Some(app_id_line), Some(name_line)) => Some(SteamGameInfo {
            name: name_line[10..name_line.len() - 1].to_string(),
            appid: app_id_line[11..app_id_line.len() - 1].to_string(),
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn parse_steam_game() {
        let string = include_str!("../testdata/acf/appmanifest_763890.acf");
        let game_info = parse_manifest_string(string);
        assert!(game_info.is_some());
        let game_info = game_info.unwrap();
        assert_eq!("Wildermyth", game_info.name);
        assert_eq!("763890", game_info.appid);
    }
}

use std::path::{Path, PathBuf};

use boilr_core::{
    config::get_backups_flder,
    steam::{get_shortcuts_paths, SteamSettings},
};
use time::{format_description, OffsetDateTime};

pub fn load_backups() -> Vec<PathBuf> {
    let backup_folder = get_backups_flder();
    let files = std::fs::read_dir(backup_folder);
    let mut result = vec![];
    if let Ok(files) = files {
        for file in files.flatten() {
            if file
                .path()
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                == "vdf"
            {
                result.push(file.path().to_path_buf());
            }
        }
    }
    result.sort();
    result.reverse();
    result
}

pub fn restore_backup(steam_settings: &SteamSettings, shortcut_path: &Path) -> bool {
    let file_name = shortcut_path.file_name();
    let paths = get_shortcuts_paths(steam_settings);
    if let (Ok(paths), Some(file_name)) = (paths, file_name) {
        for user in paths {
            if let Some(user_shortcut_path) = user.shortcut_path {
                if file_name.to_string_lossy().starts_with(&user.user_id) {
                    match std::fs::copy(shortcut_path, Path::new(&user_shortcut_path)) {
                        Ok(_) => {
                            println!("Restored shortcut to path : {user_shortcut_path}");
                        }
                        Err(err) => {
                            eprintln!(
                                "Failed to restored shortcut to path : {user_shortcut_path} gave error: {err:?}"
                            );
                        }
                    }
                    return true;
                }
            }
        }
    }
    false
}

const DATE_FORMAT: &str = "[year]-[month]-[day]-[hour]-[minute]-[second]";

pub fn backup_shortcuts(steam_settings: &SteamSettings) {
    let backup_folder = get_backups_flder();
    let paths = get_shortcuts_paths(steam_settings);
    let date = OffsetDateTime::now_utc();
    let format = format_description::parse(DATE_FORMAT);
    if let Ok(format) = format {
        let date_string = date.format(&format);
        if let (Ok(date_string), Ok(user_infos)) = (date_string, paths) {
            for user_info in user_infos {
                if let Some(shortcut_path) = user_info.shortcut_path {
                    let new_path = backup_folder.join(format!(
                        "{}-{}-shortcuts.vdf",
                        user_info.user_id, date_string
                    ));
                    match std::fs::copy(shortcut_path, &new_path) {
                        Ok(_) => {
                            println!("Backed up shortcut at: {new_path:?}");
                        }
                        Err(err) => {
                            eprintln!("Failed to backup shortcut at: {new_path:?}, error: {err:?}");
                        }
                    }
                }
            }
        }
    }
}

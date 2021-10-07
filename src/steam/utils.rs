#[cfg(target_os = "windows")]
use std::env::{self};
use std::error::Error;
use std::path::PathBuf;
use std::{fmt, path::Path};

use steam_shortcuts_util::{parse_shortcuts, shortcut::ShortcutOwned};

use super::SteamSettings;

pub fn get_shortcuts_for_user(user: &SteamUsersInfo) -> ShortcutInfo {
    let mut shortcuts = vec![];

    let new_path = match &user.shortcut_path {
        Some(shortcut_path) => {
            let content = std::fs::read(shortcut_path).unwrap();
            shortcuts = parse_shortcuts(content.as_slice())
                .unwrap()
                .iter()
                .map(|s| s.to_owned())
                .collect();
            Path::new(&shortcut_path).to_path_buf()
        }
        None => {
            println!(
                "Did not find a shortcut file for user {}, creating a new",
                user.steam_user_data_folder
            );
            let path = Path::new(&user.steam_user_data_folder).join("config");
            std::fs::create_dir_all(path.clone()).unwrap();
            path.join("shortcuts.vdf")
        }
    };

    ShortcutInfo {
        shortcuts,
        path: new_path,
    }
}

pub struct ShortcutInfo {
    pub path: PathBuf,
    pub shortcuts: Vec<ShortcutOwned>,
}

pub struct SteamUsersInfo {
    pub steam_user_data_folder: String,
    pub shortcut_path: Option<String>,
}

/// Get the paths to the steam users shortcuts (one for each user)
pub fn get_shortcuts_paths(
    settings: &SteamSettings,
) -> Result<Vec<SteamUsersInfo>, Box<dyn Error>> {
    let user_location = settings.location.clone();
    let steam_path_str = match user_location {
        Some(location) => location,
        None => get_default_location()?,
    };
    let steam_path = Path::new(&steam_path_str);
    if !steam_path.exists() {
        return Result::Err(Box::new(SteamFolderNotFound {
            location_tried: format!("{:?}", steam_path),
        }));
    }

    let user_data_path = steam_path.join("userdata");
    if !user_data_path.exists() {
        return Result::Err(Box::new(SteamFolderNotFound {
            location_tried: format!("{:?}", user_data_path),
        }));
    }

    if !user_data_path.exists() {}
    let user_folders = std::fs::read_dir(&user_data_path)?;
    let users_info = user_folders
        .filter_map(|f| f.ok())
        .map(|folder| {
            let folder_path = folder.path();
            let folder_str = folder_path
                .to_str()
                .expect("We just checked that this was there");
            let path = format!("{}//config//shortcuts.vdf", folder_str);
            let shortcuts_path = Path::new(path.as_str());
            let mut shortcuts_path_op = None;
            if shortcuts_path.exists() {
                shortcuts_path_op = Some(shortcuts_path.to_str().unwrap().to_string());
            }
            SteamUsersInfo {
                steam_user_data_folder: folder_str.to_string(),
                shortcut_path: shortcuts_path_op,
            }
        })
        .collect();
    Ok(users_info)
}

pub fn get_default_location() -> Result<String, Box<dyn Error>> {
    #[cfg(target_os = "windows")]
    let path_string = {
        let key = "PROGRAMFILES(X86)";
        let program_files = env::var(key)?;
        String::from(
            Path::new(&program_files)
                .join("Steam")
                .to_str()
                .unwrap_or(""),
        )
    };
    #[cfg(target_os = "linux")]
    let path_string = {
        let home = std::env::var("HOME")?;
        String::from(
            Path::new(&home)
                .join(".steam")
                .join("steam")
                .to_str()
                .unwrap_or(""),
        )
    };
    Ok(path_string)
}

#[derive(Debug)]
struct SteamFolderNotFound {
    location_tried: String,
}

impl fmt::Display for SteamFolderNotFound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Could not find steam user data at location: {}  Please specify it in the configuration",
            self.location_tried
        )
    }
}

impl Error for SteamFolderNotFound {
    fn description(&self) -> &str {
        self.location_tried.as_str()
    }
}

#[derive(Debug)]
struct SteamUsersDataEmpty {
    location_tried: String,
}

impl fmt::Display for SteamUsersDataEmpty {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Steam users data folder is empty: {}  Please specify it in the configuration",
            self.location_tried
        )
    }
}

impl Error for SteamUsersDataEmpty {
    fn description(&self) -> &str {
        self.location_tried.as_str()
    }
}
pub fn get_users_images(user: &SteamUsersInfo) -> Result<Vec<String>, Box<dyn Error>> {
    let grid_folder = Path::new(user.steam_user_data_folder.as_str()).join("config/grid");
    if !grid_folder.exists() {
        std::fs::create_dir_all(&grid_folder)?;
    }
    let user_folders = std::fs::read_dir(&grid_folder)?;
    let file_names = user_folders
        .filter_map(|image| image.ok())
        .map(|image| image.file_name().to_string_lossy().to_string())
        .collect();
    Ok(file_names)
}

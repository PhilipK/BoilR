use crate::platform::{Platform, SettingsValidity};
use failure::*;
use nom::bytes::complete::take_until;
use std::{
    fs::DirEntry,
    path::{Path, PathBuf},
};

use super::{origin_game::OriginGame, OriginSettings};

pub struct OriginPlatform {
    pub settings: OriginSettings,
}

impl Platform<OriginGame, OriginErrors> for OriginPlatform {
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn name(&self) -> &str {
        "Origin"
    }

    #[cfg(target_os = "linux")]
    fn create_symlinks(&self) -> bool {
        false
    }

    fn get_shortcuts(&self) -> Result<Vec<OriginGame>, OriginErrors> {
        let origin_folder = Path::new(
            &self
                .settings
                .path
                .clone()
                .unwrap_or_else(get_default_location),
        )
        .join("LocalContent");
        if !origin_folder.exists() {
            return Err(OriginErrors::PathNotFound {
                path: origin_folder.to_str().unwrap().to_string(),
            });
        }
        let game_folders =
            origin_folder
                .read_dir()
                .map_err(|e| OriginErrors::CouldNotReadGameDir {
                    path: origin_folder,
                    error: format!("{:?}", e),
                })?;
        let games = game_folders
            .filter_map(|folder| folder.ok())
            .filter_map(|game_folder| {
                let game_title = game_folder.file_name().to_string_lossy().to_string();
                let mfst_content = get_folder_mfst_file_content(&game_folder.path());
                let id = match mfst_content {
                    Some(c) => parse_id_from_file(c.as_str())
                        .ok()
                        .map(|(_, id_str)| String::from(id_str)),
                    None => None,
                };
                id.map(|id| OriginGame {
                    id,
                    title: game_title,
                })
            });
        Ok(games.collect())
    }

    fn settings_valid(&self) -> crate::platform::SettingsValidity {
        let shortcuts_res = self.get_shortcuts();
        match shortcuts_res {
            Ok(_) => SettingsValidity::Valid,
            Err(err) => SettingsValidity::Invalid {
                reason: format!("{}", err),
            },
        }
    }
}

fn get_folder_mfst_file_content(game_folder_path: &Path) -> Option<String> {
    let game_folder_files = game_folder_path.read_dir();
    if let Ok(game_folder_files) = game_folder_files {
        let mfst_file = game_folder_files
            .filter_map(|file| file.ok())
            .find(is_mfst_file)
            .map(|file| std::fs::read_to_string(&file.path()));
        return match mfst_file {
            Some(mfst_file) => mfst_file.ok(),
            None => None,
        };
    }
    None
}

fn is_mfst_file(file: &DirEntry) -> bool {
    file.path()
        .extension()
        .map(|ex| ex.to_str().unwrap_or("") == "mfst")
        .unwrap_or(false)
}

fn parse_id_from_file(i: &str) -> nom::IResult<&str, &str> {
    let (i, _) = take_until("currentstate=kReadyToStart")(i)?;
    let (i, _) = take_until("&id=")(i)?;
    let (i, _) = nom::bytes::complete::tag("&id=")(i)?;
    take_until("&")(i)
}

#[cfg(target_os = "linux")]
pub fn get_default_location() -> String {
    //TODO implement this for linux:
    // https://www.toptensoftware.com/blog/running-ea-origin-games-under-linux-via-steam-and-proton/

    //If we don't have a home drive we have to just die
    let home = std::env::var("HOME").expect("Expected a home variable to be defined");
    Path::new(&home)
        .join("Games/origin/drive_c/ProgramData/Origin/")
        .to_str()
        .unwrap()
        .to_string()
}

#[cfg(target_os = "windows")]
pub fn get_default_location() -> String {
    let key = "PROGRAMDATA";
    let program_data = std::env::var(key).expect("Expected a APPDATA variable to be defined");
    Path::new(&program_data)
        .join("Origin")
        .to_str()
        .unwrap()
        .to_string()
}

#[derive(Debug, Fail)]
pub enum OriginErrors {
    #[fail(
        display = "Origin path: {} could not be found. Try to specify a different path for Origin.",
        path
    )]
    PathNotFound { path: String },

    #[fail(
        display = "Could not read Origin directory: {:?}. Error: {}",
        path, error
    )]
    CouldNotReadGameDir { path: PathBuf, error: String },
}

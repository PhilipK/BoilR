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

    #[cfg(target_family = "unix")]
    fn create_symlinks(&self) -> bool {
        false
    }

    fn get_shortcuts(&self) -> Result<Vec<OriginGame>, OriginErrors> {
       
        let origin_folder = get_default_location();
        if  origin_folder.is_none(){
            return Err(OriginErrors::PathNotFound { path: "Default path".to_string() });
        }
        let origin_folder = origin_folder.unwrap();
        let origin_exe = origin_folder.join("Origin.exe");
        let game_folders =
            origin_folder
            .join("LocalContent")
                .read_dir()
                .map_err(|e| OriginErrors::CouldNotReadGameDir {
                    path: origin_folder.join("LocalContent"),
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
                    origin_location:Some(origin_exe.clone())
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

    fn needs_proton(&self, _input: &OriginGame) -> bool {
        #[cfg(target_os = "windows")]
        return false;
        #[cfg(target_family = "unix")]
        {
            //TODO Update this when origin gets support on linux
            return true;
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


//~/.steam/steam/steamapps/compatdata/X/pfx/drive_c/Program Files (x86)/Origin/Origin.exe
#[cfg(target_family = "unix")]
pub fn get_default_location() -> Option<PathBuf> {

    if let Ok(home)  = std::env::var("HOME"){
        let compat_folder_path = Path::new(&home)
        .join(".steam")
        .join("steam")
        .join("steamapps")
        .join("compatdata");
        if let Ok(compat_folder) = std::fs::read_dir(compat_folder_path){
            for game_folder in compat_folder {
                if let Ok(dir) = game_folder{
                   let origin_path=  dir.path()
                    .join("pfx")
                    .join("drive_c")
                    .join("Program Files (x86)")
                    .join("Origin");
                    let origin_exe = origin_path.join("Origin.exe");
                    if origin_exe.exists(){
                        return Some(origin_path.to_owned());
                    }
                }
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
pub fn get_default_location() -> Option<PathBuf> {
    let key = "PROGRAMDATA";
    let program_data = std::env::var(key);
    if let Ok(program_data) =program_data {
        let origin_folder = Path::new(&program_data)
        .join("Origin");
        if origin_folder.exsits(){
            return Some(origin_folder.to_owned());
        }
    }
    None
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

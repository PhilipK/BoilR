use crate::platforms::{
    load_settings, to_shortcuts, FromSettingsString, GamesPlatform, NeedsProton, ShortcutToImport,
};
use nom::bytes::complete::take_until;
use std::{
    fs::DirEntry,
    path::{Path, PathBuf},
};

use super::{origin_game::OriginGame, OriginSettings};

#[derive(Clone)]
pub struct OriginPlatform {
    pub settings: OriginSettings,
}

impl NeedsProton<OriginPlatform> for OriginGame {
    #[cfg(target_os = "windows")]
    fn needs_proton(&self, _platform: &OriginPlatform) -> bool {
        false
    }

    #[cfg(target_family = "unix")]
    fn needs_proton(&self, _platform: &OriginPlatform) -> bool {
        true
    }

    fn create_symlinks(&self, _platform: &OriginPlatform) -> bool {
        false
    }
}

impl OriginPlatform {
    fn get_shortcuts(&self) -> eyre::Result<Vec<OriginGame>> {
        let origin_folders =
            get_default_locations().ok_or(eyre::format_err!("Default path not found"))?;
        let origin_folder = origin_folders.local_content_path;
        let origin_exe = origin_folders.exe_path;
        let game_folders = origin_folder.join("LocalContent").read_dir()?;
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
                    origin_location: origin_exe.clone(),
                    origin_compat_folder: origin_folders.compat_folder.clone(),
                })
            });
        Ok(games.collect())
    }
}

fn get_folder_mfst_file_content(game_folder_path: &Path) -> Option<String> {
    let game_folder_files = game_folder_path.read_dir();
    if let Ok(game_folder_files) = game_folder_files {
        let mfst_file = game_folder_files
            .filter_map(|file| file.ok())
            .find(is_mfst_file)
            .map(|file| std::fs::read_to_string(file.path()));
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

#[derive(Default)]
struct OriginPathData {
    //~/.steam/steam/steamapps/compatdata/X/pfx/drive_c/Program Files (x86)/Origin/Origin.exe
    exe_path: PathBuf,
    //~/.steam/steam/steamapps/compatdata/X/pfx/drive_c/ProgramData/Origin/LocalContent
    local_content_path: PathBuf,
    //~/.steam/steam/steamapps/compatdata/X
    compat_folder: Option<PathBuf>,
}

#[cfg(target_family = "unix")]
fn get_default_locations() -> Option<OriginPathData> {
    let mut res = OriginPathData::default();
    if let Ok(home) = std::env::var("HOME") {
        let compat_folder_path = Path::new(&home)
            .join(".steam")
            .join("steam")
            .join("steamapps")
            .join("compatdata");

        if let Ok(compat_folder) = std::fs::read_dir(compat_folder_path) {
            for dir in compat_folder.flatten() {
                let origin_exe_path = dir
                    .path()
                    .join("pfx")
                    .join("drive_c")
                    .join("Program Files (x86)")
                    .join("Origin")
                    .join("Origin.exe");

                let origin_local_content = dir
                    .path()
                    .join("pfx")
                    .join("drive_c")
                    .join("ProgramData")
                    .join("Origin");

                if origin_exe_path.exists() && origin_local_content.exists() {
                    res.exe_path = origin_exe_path;
                    res.local_content_path = origin_local_content;
                    res.compat_folder = Some(dir.path());
                    return Some(res);
                }
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn get_default_locations() -> Option<OriginPathData> {
    let mut res = OriginPathData::default();
    let key = "PROGRAMDATA";
    let program_data = std::env::var(key);
    if let Ok(program_data) = program_data {
        let origin_folder = Path::new(&program_data).join("Origin");
        if origin_folder.exists() {
            res.local_content_path = origin_folder;
        } else {
            return None;
        }
        let exe_path = get_exe_path();
        match exe_path {
            Some(exe_path) => {
                res.exe_path = exe_path;
            }
            None => return None,
        }
    }
    Some(res)
}

#[cfg(target_os = "windows")]
fn get_exe_path() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;
    //Computer\HKEY_CLASSES_ROOT\eadm\shell\open\command
    RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey("eadm\\shell\\open\\command")
        .and_then(|launcher_key| launcher_key.get_value(""))
        .ok()
        .and_then(|value: String| {
            value
                .get(1..value.len() - 6)
                .map(|path_str| Path::new(path_str).to_path_buf())
        })
        .filter(|path| path.exists())
}

impl GamesPlatform for OriginPlatform {
    fn name(&self) -> &str {
        "Origin"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts(self, self.get_shortcuts())
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Origin");
        ui.checkbox(&mut self.settings.enabled, "Import from Origin");
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn code_name(&self) -> &str {
        "origin"
    }
}

impl FromSettingsString for OriginPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        OriginPlatform {
            settings: load_settings(s),
        }
    }
}

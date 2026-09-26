use crate::platforms::{
    load_settings, to_shortcuts, FromSettingsString, GamesPlatform, NeedsProton, ShortcutToImport,
};
use nom::bytes::complete::take_until;
use std::{
    fs::DirEntry,
    path::{Path, PathBuf},
};

use super::{origin_game::EAGame, EASettings};

#[derive(Clone)]
pub struct EAPlatform {
    pub settings: EASettings,
}

impl NeedsProton<EAPlatform> for EAGame {
    #[cfg(target_os = "windows")]
    fn needs_proton(&self, _platform: &EAPlatform) -> bool {
        false
    }

    #[cfg(target_family = "unix")]
    fn needs_proton(&self, _platform: &EAPlatform) -> bool {
        true
    }

    fn create_symlinks(&self, _platform: &EAPlatform) -> bool {
        false
    }
}

impl EAPlatform {
    fn get_shortcuts(&self) -> eyre::Result<Vec<EAGame>> {
        let ea_folders =
            get_default_locations().ok_or(eyre::format_err!("Default path not found"))?;
        let content_folder = ea_folders.local_content_path;
        let launcher_exe = ea_folders.exe_path;
        let game_folders = content_folder.join("LocalContent").read_dir()?;
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
                id.map(|id| EAGame {
                    id,
                    title: game_title,
                    launcher_location: launcher_exe.clone(),
                    launcher_compat_folder: ea_folders.compat_folder.clone(),
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
struct EAPathData {
    // Path to the EA Desktop / Origin launcher executable
    exe_path: PathBuf,
    // Path to the folder containing LocalContent (with .mfst game manifests)
    local_content_path: PathBuf,
    // Proton compat folder (Linux only)
    compat_folder: Option<PathBuf>,
}

#[cfg(target_family = "unix")]
fn get_default_locations() -> Option<EAPathData> {
    let mut res = EAPathData::default();
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
fn get_default_locations() -> Option<EAPathData> {
    let mut res = EAPathData::default();
    if let Ok(program_data) = std::env::var("PROGRAMDATA") {
        // EA Desktop still writes .mfst manifests here for backwards compat
        let origin_folder = Path::new(&program_data).join("Origin");
        if origin_folder.exists() {
            res.local_content_path = origin_folder;
        } else {
            return None;
        }
    } else {
        return None;
    }
    let exe_path = get_exe_path();
    match exe_path {
        Some(exe_path) => res.exe_path = exe_path,
        None => return None,
    }
    Some(res)
}

#[cfg(target_os = "windows")]
fn get_exe_path() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    // Try registry: HKEY_CLASSES_ROOT\eadm\shell\open\command
    // (registered by both old Origin and EA Desktop)
    let from_registry = RegKey::predef(HKEY_CLASSES_ROOT)
        .open_subkey("eadm\\shell\\open\\command")
        .and_then(|launcher_key| launcher_key.get_value(""))
        .ok()
        .and_then(|value: String| {
            value
                .get(1..value.len() - 6)
                .map(|path_str| Path::new(path_str).to_path_buf())
        })
        .filter(|path| path.exists());

    if from_registry.is_some() {
        return from_registry;
    }

    // Fallback: EA Desktop default install locations
    let candidate_paths: &[(&str, &str)] = &[
        ("PROGRAMFILES",     "Electronic Arts\\EA Desktop\\EA Desktop\\EADesktop.exe"),
        ("PROGRAMFILES(X86)","Electronic Arts\\EA Desktop\\EA Desktop\\EADesktop.exe"),
        ("PROGRAMFILES",     "Origin\\Origin.exe"),
        ("PROGRAMFILES(X86)","Origin\\Origin.exe"),
    ];
    for (env_var, relative) in candidate_paths {
        if let Ok(base) = std::env::var(env_var) {
            let path = Path::new(&base).join(relative);
            if path.exists() {
                return Some(path);
            }
        }
    }
    None
}

impl GamesPlatform for EAPlatform {
    fn name(&self) -> &str {
        "Origin/EA Desktop"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts(self, self.get_shortcuts())
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Origin/EA Desktop");
        ui.checkbox(&mut self.settings.enabled, "Import from Origin/EA Desktop");
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn code_name(&self) -> &str {
        // Kept as "origin" for backwards compatibility with existing settings.toml
        "origin"
    }
}

impl FromSettingsString for EAPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        EAPlatform {
            settings: load_settings(s),
        }
    }
}

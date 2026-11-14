use crate::platforms::{
    load_settings, to_shortcuts, FromSettingsString, GamesPlatform, NeedsProton, ShortcutToImport,
};
use nom::bytes::complete::take_until;
use std::{
    fs::DirEntry,
    path::{Path, PathBuf},
};

use super::{
    origin_game::{EAGame, FoundBy},
    EASettings,
};

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
        let ea_folders = get_default_locations().ok_or(eyre::format_err!(
            "Did not find the EA app or Origin. Install the EA app, or turn this source off."
        ))?;
        // Only added to on Windows, where the EA app's installer data is read.
        #[cfg_attr(not(windows), allow(unused_mut))]
        let mut games = match &ea_folders.local_content_path {
            Some(content_folder) => manifest_games(content_folder, &ea_folders)?,
            None => vec![],
        };
        // The current EA app writes no `.mfst` manifests; its games are found by their
        // installer data. A game found both ways is listed once, as its manifest has it.
        // These games start through `origin2://` links, which only the program Windows opens
        // them with handles (the EA app's EALauncher.exe); EADesktop.exe ignores them.
        #[cfg(windows)]
        let link_handler = super::installer_data::link_handler(&ea_folders.exe_path);
        #[cfg(windows)]
        let ea_app_log = std::env::var("LOCALAPPDATA").ok().map(|local| {
            Path::new(&local)
                .join("Electronic Arts")
                .join("EA Desktop")
                .join("Logs")
                .join("EADesktop.log")
        });
        #[cfg(windows)]
        for found in super::installer_data::installed_games() {
            let known = games.iter().any(|game| {
                let ids = manifest_ids(&game.id);
                found.content_ids.iter().any(|id| ids.contains(id))
            });
            if !known {
                games.push(EAGame {
                    id: found.content_ids.join(","),
                    title: found.title,
                    launcher_location: link_handler.clone(),
                    launcher_compat_folder: None,
                    found_by: FoundBy::InstallerData,
                    game_folder: Some(found.folder),
                    helper: boilr_core::launch_and_wait::helper(),
                    restart_launcher: self.settings.steam_overlay,
                    ea_app_log: ea_app_log.clone(),
                });
            }
        }
        Ok(games)
    }
}

/// The games Origin (and early versions of the EA app) listed in `.mfst` manifests.
fn manifest_games(content_folder: &Path, ea_folders: &EAPathData) -> eyre::Result<Vec<EAGame>> {
    let launcher_exe = &ea_folders.exe_path;
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
                found_by: FoundBy::Manifest,
                game_folder: None,
                helper: None,
                restart_launcher: false,
                ea_app_log: None,
            })
        });
    Ok(games.collect())
}

/// Every id in a `.mfst` id: it is URL-encoded and can name several offers
/// (`OFB-EAST%3a52735,OFB-EAST%3a1`).
#[cfg_attr(not(windows), allow(dead_code))]
fn manifest_ids(manifest_id: &str) -> Vec<String> {
    let decoded = manifest_id
        .replace("%3a", ":")
        .replace("%3A", ":")
        .replace("%2c", ",")
        .replace("%2C", ",");
    decoded
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .collect()
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
    // Path to the folder containing LocalContent (with .mfst game manifests), when there is
    // one: the current EA app no longer writes it.
    local_content_path: Option<PathBuf>,
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
                    res.local_content_path = Some(origin_local_content);
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
    // Origin and early versions of the EA app wrote .mfst manifests here; the current EA
    // app does not, so its games are found by their installer data instead.
    let local_content_path = std::env::var("PROGRAMDATA")
        .ok()
        .map(|program_data| Path::new(&program_data).join("Origin"))
        .filter(|origin_folder| origin_folder.join("LocalContent").is_dir());
    Some(EAPathData {
        exe_path: get_exe_path()?,
        local_content_path,
        compat_folder: None,
    })
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
        (
            "PROGRAMFILES",
            "Electronic Arts\\EA Desktop\\EA Desktop\\EADesktop.exe",
        ),
        (
            "PROGRAMFILES(X86)",
            "Electronic Arts\\EA Desktop\\EA Desktop\\EADesktop.exe",
        ),
        ("PROGRAMFILES", "Origin\\Origin.exe"),
        ("PROGRAMFILES(X86)", "Origin\\Origin.exe"),
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

    #[cfg(feature = "egui-ui")]
    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Origin/EA Desktop");
        ui.checkbox(&mut self.settings.enabled, "Import from Origin/EA Desktop");
        #[cfg(windows)]
        ui.checkbox(
            &mut self.settings.steam_overlay,
            "Steam overlay also when the EA app is open: close it before each game",
        );
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

#[cfg(test)]
mod manifest_tests {
    use super::manifest_ids;

    #[test]
    fn manifest_ids_are_decoded_and_split() {
        assert_eq!(
            manifest_ids("OFB-EAST%3a52735,OFB-EAST%3A1"),
            vec!["OFB-EAST:52735", "OFB-EAST:1"]
        );
        assert!(manifest_ids("").is_empty());
    }
}

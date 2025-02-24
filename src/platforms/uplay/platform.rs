//All of this is technically related to Ubisoft Connnect, not Ubisoft Play.

use std::path::Path;
use std::path::PathBuf;

use crate::platforms::load_settings;
use crate::platforms::to_shortcuts_simple;
use crate::platforms::FromSettingsString;
use crate::platforms::GamesPlatform;
use crate::platforms::NeedsProton;
use crate::platforms::ShortcutToImport;

use super::{game::UplayGame, settings::UplaySettings};

#[derive(Clone)]
pub struct UplayPlatform {
    pub settings: UplaySettings,
}

impl NeedsProton<UplayPlatform> for UplayGame {
    #[cfg(target_os = "windows")]
    fn needs_proton(&self, _platform: &UplayPlatform) -> bool {
        false
    }

    #[cfg(target_family = "unix")]
    fn needs_proton(&self, _platform: &UplayPlatform) -> bool {
        true
    }

    fn create_symlinks(&self, _platform: &UplayPlatform) -> bool {
        false
    }
}

fn get_uplay_games() -> eyre::Result<Vec<UplayGame>> {
    #[cfg(target_family = "unix")]
    {
        get_games_from_proton()
    }
    #[cfg(target_os = "windows")]
    {
        get_games_from_winreg()
    }
}

#[derive(Default)]
struct UplayPathData {
    //~/.steam/steam/steamapps/compatdata/X/pfx/drive_c/Program Files (x86)/Ubisoft/Ubisoft Game Launcher/upc.exe
    exe_path: PathBuf,
    //~/.steam/steam/steamapps/compatdata/X/pfx/drive_c/Program Files (x86)/Ubisoft/Ubisoft Game Launcher/games/
    #[cfg(target_family = "unix")]
    games_path: PathBuf,
    //~/.steam/steam/steamapps/compatdata/X
    #[cfg(target_family = "unix")]
    compat_folder: Option<PathBuf>,
}

#[cfg(target_family = "unix")]
fn get_launcher_path() -> eyre::Result<UplayPathData> {
    let mut res = UplayPathData::default();
    if let Ok(home) = std::env::var("HOME") {
        let compat_folder_path = Path::new(&home)
            .join(".steam")
            .join("steam")
            .join("steamapps")
            .join("compatdata");

        if let Ok(compat_folder) = std::fs::read_dir(compat_folder_path) {
            for dir in compat_folder.flatten() {
                let uplay_exe_path = dir
                    .path()
                    .join("pfx")
                    .join("drive_c")
                    .join("Program Files (x86)")
                    .join("Ubisoft")
                    .join("Ubisoft Game Launcher")
                    .join("upc.exe");

                let uplay_games = dir
                    .path()
                    .join("pfx")
                    .join("drive_c")
                    .join("Program Files (x86)")
                    .join("Ubisoft")
                    .join("Ubisoft Game Launcher")
                    .join("games");

                if uplay_exe_path.exists() && uplay_games.exists() {
                    res.exe_path = uplay_exe_path;
                    res.games_path = uplay_games;
                    res.compat_folder = Some(dir.path());
                    return Ok(res);
                }
            }
        }
    }
    Err(eyre::eyre!("Could not find uplay launcher"))
}

#[cfg(target_os = "windows")]
fn get_launcher_path() -> eyre::Result<UplayPathData> {
    let mut res = UplayPathData::default();
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let launcher_key = hklm.open_subkey("SOFTWARE\\WOW6432Node\\Ubisoft\\Launcher")?;
    let launcher_dir: String = launcher_key.get_value("InstallDir")?;
    let path = Path::new(&launcher_dir).join("upc.exe");
    if path.exists() {
        res.exe_path = path;
        Ok(res)
    } else {
        Err(eyre::eyre!(
            "Could not find uplay launcher at path {:?}",
            path
        ))
    }
}

#[cfg(target_os = "windows")]
fn get_games_from_winreg() -> eyre::Result<Vec<UplayGame>> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut games = vec![];
    let mut installed_ids = vec![];
    let launcher_path = get_launcher_path()?.exe_path;

    if let Ok(installs) = hklm.open_subkey("SOFTWARE\\WOW6432Node\\Ubisoft\\Launcher\\Installs") {
        for i in installs.enum_keys().filter_map(|i| i.ok()) {
            if let Ok(install) = installs.open_subkey(&i) {
                let install_dir: Result<String, _> = install.get_value("InstallDir");
                if let Ok(folder) = install_dir {
                    let path = Path::new(&folder);
                    if path.exists() {
                        installed_ids.push(i);
                    }
                }
            }
        }
    }

    for id in installed_ids {
        let path = format!("SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\Uplay Install {}",id);
        let subkey = hklm.open_subkey(path);
        if let Ok(subkey) = subkey {
            let name: Result<String, _> = subkey.get_value("DisplayName");
            if let Ok(name) = name {
                let icon: String = subkey.get_value("DisplayIcon").unwrap_or_default();
                games.push(UplayGame {
                    name,
                    icon,
                    id,
                    launcher: launcher_path.clone(),
                    launcher_compat_folder: None,
                    launch_id: 0,
                })
            }
        }
    }

    Ok(games)
}

#[cfg(target_family = "unix")]
fn get_games_from_proton() -> eyre::Result<Vec<UplayGame>> {

    let launcher_path = get_launcher_path()?;
    let parent = launcher_path
        .exe_path
        .parent()
        .unwrap_or_else(|| Path::new("/"));
    let file = parent
        .join("cache")
        .join("configuration")
        .join("configurations");
    let buffer = std::fs::read(file)?;
    let splits = get_file_splits(&buffer);
    let configurations = splits.iter().filter(|s| is_valid_game_config(s));
    let parsed_configurations= configurations.flat_map(|config| parse_game_config(config));
    let games = parsed_configurations.map(|game|{
        UplayGame{
            name:game.shortcut_name.to_string(),
            icon: parent.join("data").join("games").join(game.icon_image).to_string_lossy().to_string(),
            id : game.register
                    .strip_prefix("HKEY_LOCAL_MACHINE\\SOFTWARE\\Ubisoft\\Launcher\\Installs\\")
                    .unwrap_or_default()
                    .strip_suffix("\\InstallDir")
                    .unwrap_or_default()
                    .to_string(),
            launcher : launcher_path.exe_path.clone(),
            launcher_compat_folder: launcher_path.compat_folder.clone(),
            launch_id: game.launch_id
        }
    });
    Ok(games.collect())
}

struct GameConfig<'a> {
    icon_image: &'a str,
    shortcut_name: &'a str,
    register: &'a str,
    launch_id: usize,
}

fn parse_game_config(split: &str) -> Vec<GameConfig> {
    let mut res = vec![];
    let mut icon_image = "";
    let mut shortcut_name = "";
    let mut register = "";
    let mut inonline = false;
    let mut launch_id = 0;
    for line in split.lines().map(|line| line.trim()) {
        if line.starts_with("online:") {
            inonline = true;
            continue;
        }
        if line.starts_with("offline:") {
            break;
        }
        if let Some(split) = line.strip_prefix("icon_image: ") {
            if split.is_empty() {
                break;
            }; // invalid config.
            icon_image = split;
        }
        if !inonline {
            continue;
        };
        if let Some(split) = line.strip_prefix("- shortcut_name: ") {
            if split.is_empty() {
                break;
            }; // invalid config.
            shortcut_name = split;
            continue;
        }

        if let Some(split) = line.strip_prefix("register: ") {
            if split.is_empty() {
                break;
            }; // invalid config.
            register = split;
            continue;
        }

        if line == "denuvo: yes" {
            res.push(GameConfig {
                icon_image,
                shortcut_name,
                register,
                launch_id,
            });
            launch_id += 1;
        }
    }
    res
}

fn is_valid_game_config(config: &str) -> bool {
    let requires = ["executables:", "online:", "shortcut_name:", "register:"];
    requires.iter().all(|req| config.contains(req))
}

fn get_file_splits(buffer: &[u8]) -> Vec<String> {
    let new_string = String::from_utf8_lossy(buffer);
    let sections = new_string.split("version: 2.0").map(|s| s.replace('�', ""));
    sections.collect()
}

impl FromSettingsString for UplayPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        UplayPlatform {
            settings: load_settings(s),
        }
    }
}

impl GamesPlatform for UplayPlatform {
    fn name(&self) -> &str {
        "Uplay"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts_simple(get_uplay_games())
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Uplay");
        ui.checkbox(&mut self.settings.enabled, "Import from Uplay");
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn code_name(&self) -> &str {
        "uplay"
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn can_parse_configuration_file() {
        let content = include_bytes!("testconfiguration");
        let splits = get_file_splits(content);
        assert_eq!(501, splits.len());
    }

   #[test]
    fn can_parse_into_game_config() {
        let content = include_bytes!("testconfiguration");
        let splits = get_file_splits(content);
        let games:Vec<_> = splits.iter().flat_map(|split| parse_game_config(split)).collect();
        assert_eq!(2, games.len());
        assert_eq!(Some("For Honor"),games.get(0).map(|h|h.shortcut_name));
        assert_eq!(Some("WATCH_DOGS® 2"),games.get(1).map(|h|h.shortcut_name));
    }
}

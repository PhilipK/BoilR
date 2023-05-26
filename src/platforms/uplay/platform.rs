//All of this is technically related to Ubisoft Connnect, not Ubisoft Play.

use std::path::Path;
use std::path::PathBuf;

use std::io::Read;
use std::io::BufReader;
use std::fs::File;

use crate::platforms::load_settings;
use crate::platforms::to_shortcuts_simple;
use crate::platforms::FromSettingsString;
use crate::platforms::GamesPlatform;
use crate::platforms::ShortcutToImport;
use crate::platforms::NeedsPorton;

use super::{game::UplayGame, settings::UplaySettings};

#[derive(Clone)]
pub struct UplayPlatform {
    pub settings: UplaySettings,
}

impl NeedsPorton<UplayPlatform> for UplayGame {
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
    games_path: PathBuf,
    //~/.steam/steam/steamapps/compatdata/X
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
    return Err(eyre::eyre!(
        "Could not find uplay launcher"))
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
                    launcher_compat_folder: None(),
                    launch_id: 0
                })
            }
        }
    }

    Ok(games)
}

#[cfg(target_family = "unix")]
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|window| 
        window == needle)
}

#[cfg(target_family = "unix")]
fn get_games_from_proton() -> eyre::Result<Vec<UplayGame>> {
    let mut games = vec![];

    let launcher_path = get_launcher_path()?;
    let file = File::open(launcher_path.exe_path.parent().unwrap()
    .join("cache")
    .join("configuration")
    .join("configurations"))?;
    let mut reader = BufReader::new(file);
    let mut buffer = Vec::new();
    
    // Read file into vector.
    reader.read_to_end(&mut buffer)?;

    let mut splits: Vec<String> = Vec::new();

    while buffer.len() > 0 {
        let foundindex: usize;
        match find_subsequence(buffer.as_slice(), b"version: 2.0") {
            Some(index) => {foundindex = index},
            None => {break;},
        };
        let (mut first, second) = buffer.split_at(foundindex);
        if first.len() >= 14usize {
            first = first.split_at(first.len()-14).0;
        }
        splits.push(unsafe {std::str::from_utf8_unchecked(first).to_string()});
        buffer = second.split_at("version: 2.0".len()).1.to_vec();
    }

    
    

    for gameconfig in splits {
        if !gameconfig.contains("executables:") {continue};
        if !gameconfig.contains("online:") {continue};
        if !gameconfig.contains("shortcut_name:") {continue};
        if !gameconfig.contains("register:") {continue};

        let mut inonline = false;
        let mut shortcut_name: String = "".to_string();
        let mut game_id: String = "".to_string();
        let mut icon_image: PathBuf = "".into();
        let mut launch_id = 0;
        for line in gameconfig.split('\n') {
            let trimed = line.trim();
            if trimed.starts_with("online:") {
                inonline = true;
                continue;
            }
            if trimed.starts_with("offline:") {
                break;
            }
            if trimed.starts_with("icon_image: ") {
                let split = trimed.split_at(trimed.find(": ").expect("Couldn't find icon_image value!")+2).1;
                if split.len() == 0usize  {break}; // invalid config.
                icon_image = launcher_path.exe_path.parent().unwrap().join("data").join("games").join(split);
            }
            if !inonline {continue};
            if trimed.starts_with("- shortcut_name:") {
                let split = trimed.split_at(trimed.find(": ").expect("Couldn't find shortcut_name value!")+2).1;
                if split.len() == 0usize  {break}; // invalid config.
                shortcut_name = split.to_string();
                continue;
            }

            if trimed.starts_with("register: ") {
                let split = trimed.split_at(trimed.find(": ").expect("Couldn't find register value!")+2).1;
                if split.len() == 0usize  {break}; // invalid config.
                game_id = split.to_string()
                .strip_prefix("HKEY_LOCAL_MACHINE\\SOFTWARE\\Ubisoft\\Launcher\\Installs\\").expect("Game register dind't start with expected value!")
                .strip_suffix("\\InstallDir").expect("Game register dind't end with expected value!").to_string();
                continue;
            }

            if trimed == "denuvo: yes" {
                games.push(UplayGame {
                    name: shortcut_name.clone(),
                    icon: icon_image.to_str().unwrap().to_string(),
                    id: game_id.clone(),
                    launcher: launcher_path.exe_path.clone(),
                    launcher_compat_folder: launcher_path.compat_folder.clone(),
                    launch_id,
                });
                launch_id = launch_id + 1;
            }
        }
    }
    Ok(games)
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

    fn get_settings_serilizable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn code_name(&self) -> &str {
        "uplay"
    }
}

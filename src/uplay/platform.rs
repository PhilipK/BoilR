use crate::platform::Platform;
use std::error::Error;
use std::path::Path;
use std::path::PathBuf;

use super::{game::Game, settings::UplaySettings};

pub struct Uplay {
    pub settings: UplaySettings,
}

impl Platform<Game, Box<dyn Error>> for Uplay {
    fn enabled(&self) -> bool {
        #[cfg(target_family = "unix")]
        {
            false
        }
        #[cfg(target_os = "windows")]
        {
            self.settings.enabled
        }
    }

    fn name(&self) -> &str {
        "Uplay"
    }

    fn settings_valid(&self) -> crate::platform::SettingsValidity {
        #[cfg(target_family = "unix")]
        {
            //Linux not supported yet
            crate::platform::SettingsValidity::Invalid {
                reason: "Linux not supported yet".to_string(),
            }
        }
        #[cfg(target_os = "windows")]
        {
            if get_launcher_path().is_some() {
                crate::platform::SettingsValidity::Valid
            } else {
                crate::platform::SettingsValidity::Invalid {
                    reason: "Could not find UPlay instalation".to_string(),
                }
            }
        }
    }

    fn get_shortcuts(&self) -> Result<Vec<Game>, Box<dyn Error>> {
        #[cfg(target_family = "unix")]
        {
            Ok(vec![])
        }
        #[cfg(target_os = "windows")]
        {
            get_games_from_winreg()
        }
    }

    #[cfg(target_family = "unix")]
    fn create_symlinks(&self) -> bool {
        false
    }

    fn needs_proton(&self, _input: &Game) -> bool {
        #[cfg(target_os = "windows")]
        return false;
        #[cfg(target_family = "unix")]
        {
            //TODO update this when uplay gets proton support on linux
            true
        }
    }
}
#[cfg(target_os = "windows")]
fn get_launcher_path() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    if let Ok(launcher_key) = hklm.open_subkey("SOFTWARE\\WOW6432Node\\Ubisoft\\Launcher") {
        let launcher_dir: Result<String, _> = launcher_key.get_value("InstallDir");
        if let Ok(launcher_dir) = launcher_dir {
            let path = Path::new(&launcher_dir).join("upc.exe");
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn get_games_from_winreg() -> Result<Vec<Game>, Box<dyn Error>> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut games = vec![];
    let mut installed_ids = vec![];
    let launcher_path =
        get_launcher_path().expect("This should only be called if launcher is found");

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
                games.push(Game {
                    name,
                    icon,
                    id,
                    launcher: launcher_path.clone(),
                })
            }
        }
    }

    Ok(games)
}

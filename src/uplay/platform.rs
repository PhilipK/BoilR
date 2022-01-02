use crate::platform::Platform;
use std::error::Error;

use super::{game::Game, settings::UplaySettings};

pub struct Uplay {
    pub settings: UplaySettings,
}

impl Platform<Game, Box<dyn Error>> for Uplay {
    fn enabled(&self) -> bool {
        #[cfg(target_os = "linux")]
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
        crate::platform::SettingsValidity::Valid
    }

    fn get_shortcuts(&self) -> Result<Vec<Game>, Box<dyn Error>> {
        #[cfg(target_os = "linux")]
        {
            Ok(vec![])
        }
        #[cfg(target_os = "windows")]
        {
            get_games_from_winreg()
        }
    }

    #[cfg(target_os = "linux")]
    fn create_symlinks(&self) -> bool {
        false
    }
}

#[cfg(target_os = "windows")]
fn get_games_from_winreg() -> Result<Vec<Game>, Box<dyn Error>> {
    use std::path::Path;

    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut games = vec![];
    let mut installed_ids = vec![];
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
                games.push(Game { name, icon, id })
            }
        }
    }

    Ok(games)
}

#[cfg(target_os = "windows")]
use std::path::Path;
#[cfg(target_os = "windows")]
use std::path::PathBuf;

use super::{game::Game, settings::UplaySettings};

#[derive(Clone)]
pub struct UplayPlatform {
    pub settings: UplaySettings,
}

pub(crate) fn get_uplay_games() -> eyre::Result<Vec<Game>> {
    #[cfg(target_family = "unix")]
    {
        Ok(vec![])
    }
    #[cfg(target_os = "windows")]
    {
        get_games_from_winreg()
    }
}

#[cfg(target_os = "windows")]
fn get_launcher_path() -> eyre::Result<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let launcher_key = hklm.open_subkey("SOFTWARE\\WOW6432Node\\Ubisoft\\Launcher")?;
    let launcher_dir: String = launcher_key.get_value("InstallDir")?;
    let path = Path::new(&launcher_dir).join("upc.exe");
    if path.exists() {
        Ok(path)
    } else {
        Err(eyre::eyre!(
            "Could not find uplay launcher at path {:?}",
            path
        ))
    }
}

#[cfg(target_os = "windows")]
fn get_games_from_winreg() -> eyre::Result<Vec<Game>> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut games = vec![];
    let mut installed_ids = vec![];
    let launcher_path = get_launcher_path()?;

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

impl UplayPlatform {
    pub(crate) fn render_uplay_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Uplay");
        ui.checkbox(&mut self.settings.enabled, "Import from Uplay");        
    }
}

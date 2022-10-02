#[cfg(target_os = "windows")]
use std::path::Path;
#[cfg(target_os = "windows")]
use std::path::PathBuf;

use crate::platforms::FromSettingsString;
use crate::platforms::GamesPlatform;
use crate::platforms::load_settings;
use crate::platforms::to_shortcuts_simple;
use crate::platforms::ShortcutToImport;

use super::{game::UplayGame, settings::UplaySettings};

#[derive(Clone)]
pub struct UplayPlatform {
    pub settings: UplaySettings,
}

fn get_uplay_games() -> eyre::Result<Vec<UplayGame>> {
    #[cfg(target_family = "unix")]
    {
        Err(eyre::format_err!("Uplay is not supported on Linux"))
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
fn get_games_from_winreg() -> eyre::Result<Vec<UplayGame>> {
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
                games.push(UplayGame {
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

impl FromSettingsString for UplayPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        UplayPlatform {
            settings: load_settings(s),
        }
    }
}


impl GamesPlatform for UplayPlatform{
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
use super::{HeroicGame, HeroicSettings};
use crate::platform::{Platform, SettingsValidity};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;
use std::process::Command;

#[cfg(target_os = "windows")]
use std::path::PathBuf;

pub struct HeroicPlatform {
    pub settings: HeroicSettings,
}

#[cfg(target_family = "unix")]
enum InstallationMode {
    FlatPak,
    UserBin,
}

#[cfg(target_family = "unix")]
fn get_config_folder(install_mode: &InstallationMode) -> Option<String> {
    match install_mode {
        InstallationMode::FlatPak => {
            let home_dir = std::env::var("HOME").unwrap_or("".to_string());
            Some(
                Path::new(&home_dir)
                    .join(".var/app/com.heroicgameslauncher.hgl/config")
                    .to_string_lossy()
                    .to_string(),
            )
        }
        //TODO Fix this to find the acutal config folder when it is installed with userbin 
        InstallationMode::UserBin => None,
    }
}

#[cfg(target_os = "windows")]
fn heroic_folder_from_registry() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(launcher) = hklm.open_subkey("Software\\035fb1f9-7381-565b-92bb-ed6b2a3b99ba") {
        let path_string: Result<String, _> = launcher.get_value("InstallLocation");
        if let Ok(path_string) = path_string {
            let path = Path::new(&path_string);
            if path.exists() {
                return Some(path.to_path_buf());
            }
        }
    }
    None
}

// TODO update this to find the manifest files when proton works
#[cfg(target_os = "windows")]
fn heroic_folder_from_appdata() -> Option<PathBuf> {
    let key = "APPDATA";
    match std::env::var(key) {
        Ok(program_data) => {
            let path = Path::new(&program_data).join("heroic");
            if path.exists() {
                Some(path.to_path_buf())
            } else {
                None
            }
        }
        Err(_err) => None,
    }
}

#[cfg(target_family = "unix")]
fn get_shortcuts_from_install_mode(
    install_mode: &InstallationMode,
) -> Result<Vec<HeroicGame>, Box<dyn Error>> {
    let config_folder = get_config_folder(install_mode);
    get_shortcuts_from_location(config_folder)
}


//~/.var/app/com.heroicgameslauncher.hgl/config/legendary/installed.json

fn get_shortcuts_from_location(
    config_folder: Option<String>,
) -> Result<Vec<HeroicGame>, Box<dyn Error>> {

    if let Some(config_folder) = config_folder{
        let installed_json_path = Path::new(&config_folder).join("legendary").join("installed.json");
        if installed_json_path.exists(){
            let json = std::fs::read_to_string(installed_json_path)?;
            let games_map = serde_json::from_str::<HashMap<String,HeroicGame>>(&json)?;
            let mut games = vec![];
            for game in games_map.values(){
                games.push(game.clone());
            }
            return Ok( games );
        }
    }
    //TODO Should error be returned instead?
    return Ok(vec![]);
}

impl Platform<HeroicGame, Box<dyn Error>> for HeroicPlatform {
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn name(&self) -> &str {
        "Heroic"
    }
    #[cfg(target_family = "unix")]
    fn get_shortcuts(&self) -> Result<Vec<HeroicGame>, Box<dyn Error>> {
        let install_modes = vec![InstallationMode::FlatPak, InstallationMode::UserBin];
        let first_working_instal = install_modes
            .iter()
            .find_map(|install_mode| get_shortcuts_from_install_mode(install_mode).ok());
        match first_working_instal {
            Some(res) => return Ok(res),
            None => get_shortcuts_from_install_mode(&install_modes[0]),
        }
    }

    #[cfg(target_os = "windows")]
    fn get_shortcuts(&self) -> Result<Vec<HeroicGame>, Box<dyn Error>> {
        let legendary = find_legendary_location().unwrap_or("legendary".to_string());
        get_shortcuts_from_location(None, legendary)
    }

    #[cfg(target_family = "unix")]
    fn create_symlinks(&self) -> bool {
        false
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

    fn needs_proton(_input: &HeroicGame) -> bool {
        #[cfg(target_os = "windows")]
        return false;
        #[cfg(target_family = "unix")]
        {
            //TODO update this when Heroic is updated
            return false;
        }        
    }
}

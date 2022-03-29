use super::{HeroicGame, HeroicSettings};
use crate::platform::{Platform, SettingsValidity};
use serde_json::from_str;
use std::error::Error;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

pub struct HeroicPlatform {
    pub settings: HeroicSettings,
}

#[cfg(target_family = "unix")]
enum InstallationMode {
    FlatPak,
    UserBin,
}

#[cfg(target_family = "unix")]
fn get_legendary_location(install_mode: &InstallationMode) -> &'static str {
    match install_mode{
        InstallationMode::FlatPak => "/var/lib/flatpak/app/com.heroicgameslauncher.hgl/current/active/files/bin/heroic/resources/app.asar.unpacked/build/bin/linux/legendary",
        InstallationMode::UserBin => "/opt/Heroic/resources/app.asar.unpacked/build/bin/linux/legendary"
    }
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
        InstallationMode::UserBin => None,
    }
}

#[cfg(target_os = "windows")]
fn find_legendary_location() -> Option<String> {
    match heroic_folder_from_registry().or_else(heroic_folder_from_appdata) {
        Some(heroic_folder) => {
            let legendary_path = heroic_folder
                .join("resources\\app.asar.unpacked\\build\\bin\\win32\\legendary.exe");
            if legendary_path.exists() {
                Some(legendary_path.to_path_buf().to_string_lossy().to_string())
            } else {
                None
            }
        }
        None => None,
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
            //.join("resources/app.asar.unpacked/build/bin/win32/legendary.exe")
            let path = Path::new(&path_string);
            if path.exists() {
                return Some(path.to_path_buf());
            }
        }
    }
    None
}

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
    let legendary = get_legendary_location(install_mode);
    let config_folder = get_config_folder(install_mode);
    get_shortcuts_from_location(config_folder, legendary.to_string())
}

fn get_shortcuts_from_location(
    config_folder: Option<String>,
    legendary: String,
) -> Result<Vec<HeroicGame>, Box<dyn Error>> {
    let output = if let Some(config_folder) = config_folder.clone() {
        Command::new(&legendary)
            .env("XDG_CONFIG_HOME", config_folder)
            .arg("list-installed")
            .arg("--json")
            .output()?
    } else {
        Command::new(&legendary)
            .arg("list-installed")
            .arg("--json")
            .output()?
    };
    let json = String::from_utf8_lossy(&output.stdout);
    let mut legendary_ouput: Vec<HeroicGame> = from_str(&json)?;
    legendary_ouput.iter_mut().for_each(|mut game| {
        game.config_folder = config_folder.clone();
        game.legendary_location = Some(legendary.to_string());
    });
    Ok(legendary_ouput)
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

    fn needs_proton(input: &HeroicGame) -> bool {
        #[cfg(target_os = "windows")]
        return false;
        #[cfg(target_family = "unix")]
        {
            //TODO update this when Heroic is updated
            return false;
        }        
    }
}

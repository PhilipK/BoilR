use super::{HeroicGame, HeroicSettings};
use crate::platform::{Platform, SettingsValidity};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use std::path::PathBuf;

pub struct HeroicPlatform {
    pub settings: HeroicSettings,
}

enum InstallationMode {
    FlatPak,
    UserBin,    
}

fn get_installed_json_location(install_mode: &InstallationMode) -> PathBuf {
    let home_dir = std::env::var("HOME").unwrap_or("".to_string());
    match install_mode {
        InstallationMode::FlatPak => Path::new(&home_dir)
            .join(".var/app/com.heroicgameslauncher.hgl/config/legendary/installed.json"),
        InstallationMode::UserBin => Path::new(&home_dir).join(".config/legendary/installed.json"),
    }
    .to_path_buf()
}



fn get_shortcuts_from_install_mode(
    install_mode: &InstallationMode,
) -> Result<Vec<HeroicGame>, Box<dyn Error>> {
    let installed_path = get_installed_json_location(install_mode);
    get_shortcuts_from_location(installed_path)
}

fn get_shortcuts_from_location<P: AsRef<Path>>(path: P) -> Result<Vec<HeroicGame>, Box<dyn Error>> {
    let installed_json_path = path.as_ref();
    if installed_json_path.exists() {
        let json = std::fs::read_to_string(installed_json_path)?;
        let games_map = serde_json::from_str::<HashMap<String, HeroicGame>>(&json)?;
        let mut games = vec![];
        for game in games_map.values() {
            games.push(game.clone());
        }
        return Ok(games);
    }
    return Ok(vec![]);
}

impl Platform<HeroicGame, Box<dyn Error>> for HeroicPlatform {
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn name(&self) -> &str {
        "Heroic"
    }
    fn get_shortcuts(&self) -> Result<Vec<HeroicGame>, Box<dyn Error>> {        
        let install_modes = vec![InstallationMode::FlatPak, InstallationMode::UserBin];
        let mut shortcuts :Vec<HeroicGame> = install_modes
            .iter()
            .filter_map(|install_mode| get_shortcuts_from_install_mode(install_mode).ok())
            .flatten()
            .collect();

        shortcuts.sort_by_key(|m| format!("{}-{}-{}",m.launch_parameters,m.executable,&m.app_name));
        shortcuts.dedup_by_key(|m| format!("{}-{}-{}",m.launch_parameters,m.executable,&m.app_name));

        Ok(shortcuts)
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

    fn needs_proton(&self, _input: &HeroicGame) -> bool {  
        return true;
    }
}

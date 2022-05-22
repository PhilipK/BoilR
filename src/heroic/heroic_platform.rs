use serde::Deserialize;

use super::{HeroicGame, HeroicGameType, HeroicSettings};
use crate::gog::get_shortcuts_from_game_folders;
use crate::platform::{Platform, SettingsValidity};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use std::path::PathBuf;

pub struct HeroicPlatform {
    pub settings: HeroicSettings,
}

#[derive(Deserialize, Debug, Clone)]
pub enum InstallationMode {
    FlatPak,
    UserBin,
}

#[derive(Deserialize)]
struct HeroicGogConfig {
    installed: Vec<HeroicGogPath>,
}

#[derive(Deserialize)]
struct HeroicGogPath {
    platform: String,
    #[serde(alias = "appName")]
    app_name: String,
    install_path: String,
}

fn get_installed_json_location(install_mode: &InstallationMode) -> PathBuf {
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "".to_string());
    match install_mode {
        InstallationMode::FlatPak => Path::new(&home_dir)
            .join(".var/app/com.heroicgameslauncher.hgl/config/legendary/installed.json"),
        InstallationMode::UserBin => Path::new(&home_dir).join(".config/legendary/installed.json"),
    }
}

fn get_gog_installed_location(install_mode: &InstallationMode) -> PathBuf {
    let home_dir = std::env::var("HOME").unwrap_or_else(|_| "".to_string());
    match install_mode {
        InstallationMode::FlatPak => Path::new(&home_dir)
            .join(".var/app/com.heroicgameslauncher.hgl/config/heroic/gog_store/installed.json"),
        InstallationMode::UserBin => {
            Path::new(&home_dir).join(".config/heroic/gog_store/installed.json")
        }
    }
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
        Ok(games)
    } else {
        Ok(vec![])
    }
}

impl Platform<HeroicGameType, Box<dyn Error>> for HeroicPlatform {
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn name(&self) -> &str {
        "Heroic"
    }
    fn get_shortcuts(&self) -> Result<Vec<HeroicGameType>, Box<dyn Error>> {
        let install_modes = vec![InstallationMode::FlatPak, InstallationMode::UserBin];

        let mut heroic_games = self.get_epic_games(&install_modes)?;
        if let Ok(gog_games) = get_gog_games(&install_modes) {
            heroic_games.extend(gog_games);
        } else {
            println!("Did not find any GOG games in heroic")
        }
        Ok(heroic_games)
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

    fn needs_proton(&self, input: &HeroicGameType) -> bool {
        match input {
            HeroicGameType::Epic(_) => true,
            HeroicGameType::Gog(_, is_windows) => *is_windows,
        }
    }
}

impl HeroicPlatform {
    fn get_epic_games(
        &self,
        install_modes: &[InstallationMode],
    ) -> Result<Vec<HeroicGameType>, Box<dyn Error>> {
        let mut shortcuts: Vec<HeroicGame> = install_modes
            .iter()
            .filter_map(|install_mode| {
                let mut shortcuts = get_shortcuts_from_install_mode(install_mode).ok();
                if let Some(shortcuts) = shortcuts.as_mut() {
                    for shortcut in shortcuts {
                        shortcut.install_mode = Some(install_mode.clone());
                        shortcut.launch_directly = !self.settings.launch_games_through_heroic;
                    }
                }
                shortcuts
            })
            .flatten()
            .filter(|s| s.is_installed())
            .collect();

        shortcuts
            .sort_by_key(|m| format!("{}-{}-{}", m.launch_parameters, m.executable, &m.app_name));
        shortcuts
            .dedup_by_key(|m| format!("{}-{}-{}", m.launch_parameters, m.executable, &m.app_name));
        let mut epic_shortcuts = vec![];
        for shortcut in shortcuts {
            epic_shortcuts.push(HeroicGameType::Epic(shortcut));
        }
        Ok(epic_shortcuts)
    }
}

fn get_gog_games(
    install_modes: &[InstallationMode],
) -> Result<Vec<HeroicGameType>, Box<dyn Error>> {
    let gog_paths: Vec<HeroicGogPath> = install_modes
        .iter()
        .filter_map(|install_mode| {
            let config = get_gog_installed_location(install_mode);
            if config.exists() {
                Some(config)
            } else {
                None
            }
        })
        .filter_map(|config_path| std::fs::read_to_string(config_path).ok())
        .filter_map(|config_string| serde_json::from_str::<HeroicGogConfig>(&config_string).ok())
        .flat_map(|config| config.installed)
        .collect();

    let mut is_windows_map = HashMap::new();

    for path in gog_paths.iter() {
        is_windows_map.insert(path.app_name.clone(), path.platform == "windows");
    }

    let game_folders = gog_paths
        .iter()
        .filter_map(|p| {
            let path = Path::new(&p.install_path);
            if path.exists() {
                Some(path.to_path_buf())
            } else {
                None
            }
        })
        .collect();
    let shortcuts = get_shortcuts_from_game_folders(game_folders);
    let mut gog_shortcuts = vec![];
    for shortcut in shortcuts {
        let is_windows = is_windows_map.get(&shortcut.game_id).unwrap_or(&false);
        gog_shortcuts.push(HeroicGameType::Gog(shortcut, *is_windows));
    }

    Ok(gog_shortcuts)
}

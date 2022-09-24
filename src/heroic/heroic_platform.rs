use serde::Deserialize;

use super::{HeroicGame, HeroicGameType, HeroicSettings};
use crate::platforms::{Platform, SettingsValidity};
use std::collections::HashMap;
use std::error::Error;
use std::path::Path;

use std::path::PathBuf;

pub struct HeroicPlatform {
    pub settings: HeroicSettings,
}

#[derive(Deserialize, Debug, Clone, Copy)]
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

impl HeroicPlatform {
    pub fn get_heroic_games(&self) -> Vec<HeroicGameType> {
        let install_modes = vec![InstallationMode::FlatPak, InstallationMode::UserBin];

        let mut heroic_games = self.get_epic_games(&install_modes);
        if let Ok(gog_games) = get_gog_games(&self.settings, &install_modes) {
            heroic_games.extend(gog_games);
        } else {
            println!("Did not find any GOG games in heroic")
        }
        heroic_games
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
        Ok(self.get_heroic_games())
    }

    #[cfg(target_family = "unix")]
    fn create_symlinks(&self) -> bool {
        false
    }

    fn settings_valid(&self) -> crate::platforms::SettingsValidity {
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
            HeroicGameType::Epic(_game) => true,
            HeroicGameType::Gog(_, is_windows) => *is_windows,
            HeroicGameType::Heroic { .. } => false,
        }
    }
}

impl HeroicPlatform {
    pub fn get_epic_games(&self, install_modes: &[InstallationMode]) -> Vec<HeroicGameType> {
        let mut shortcuts = vec![];
        for install_mode in install_modes {
            if let Ok(mut games) = get_shortcuts_from_install_mode(install_mode) {
                games.sort_by_key(|m| {
                    format!("{}-{}-{}", m.launch_parameters, m.executable, &m.app_name)
                });
                games.dedup_by_key(|m| {
                    format!("{}-{}-{}", m.launch_parameters, m.executable, &m.app_name)
                });

                for game in games {
                    if self.settings.is_heroic_launch(&game.app_name) {
                        shortcuts.push(HeroicGameType::Heroic {
                            title: game.title,
                            app_name: game.app_name,
                            install_mode: *install_mode,
                        });
                    } else {
                        if game.is_installed() {
                            shortcuts.push(HeroicGameType::Epic(game));
                        }
                    }
                }
            }
        }
        shortcuts
    }
}
fn get_gog_games(
    settings: &HeroicSettings,
    install_modes: &[InstallationMode],
) -> Result<Vec<HeroicGameType>, Box<dyn Error>> {
    let mut gog_paths = vec![];
    for install_mode in install_modes {
        let config = get_gog_installed_location(install_mode);
        if config.exists() {
            if let Ok(config_string) = std::fs::read_to_string(config) {
                if let Ok(config) = serde_json::from_str::<HeroicGogConfig>(&config_string) {
                    for c in config.installed {
                        gog_paths.push((install_mode, c));
                    }
                }
            }
        }
    }

    let mut is_windows_map = HashMap::new();

    for (_, path) in gog_paths.iter() {
        is_windows_map.insert(path.app_name.clone(), path.platform == "windows");
    }

    let mut gog_shortcuts = vec![];

    let heroic_games = gog_paths
        .iter()
        .filter(|(_, p)| settings.is_heroic_launch(&p.app_name))
        .filter_map(|(install_mode, p)| {
            let path = Path::new(&p.install_path);
            if path.exists() {
                let title = path.file_name();
                Some(HeroicGameType::Heroic {
                    title: title.unwrap_or_default().to_string_lossy().to_string(),
                    app_name: p.app_name.clone(),
                    install_mode: **install_mode,
                })
            } else {
                None
            }
        });
    gog_shortcuts.extend(heroic_games);

    let game_folders = gog_paths
        .iter()
        .filter(|(_, p)| !settings.is_heroic_launch(&p.app_name))
        .filter_map(|(_, p)| {
            let path = Path::new(&p.install_path);
            if path.exists() {
                Some(path.to_path_buf())
            } else {
                None
            }
        })
        .collect();
    let direct_shortcuts = crate::platforms::get_gog_shortcuts_from_game_folders(game_folders);
    for shortcut in direct_shortcuts {
        let is_windows = is_windows_map.get(&shortcut.game_id).unwrap_or(&false);
        gog_shortcuts.push(HeroicGameType::Gog(shortcut, *is_windows));
    }

    Ok(gog_shortcuts)
}

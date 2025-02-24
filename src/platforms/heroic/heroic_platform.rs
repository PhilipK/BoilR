use serde::Deserialize;

use super::{HeroicGame, HeroicGameType, HeroicSettings};
use crate::platforms::{load_settings, FromSettingsString, GamesPlatform};
use crate::platforms::{to_shortcuts, NeedsProton, ShortcutToImport};
use std::collections::HashMap;
use std::path::Path;

use std::path::PathBuf;

#[derive(Clone)]
pub struct HeroicPlatform {
    pub settings: HeroicSettings,
    pub(crate) heroic_games: Option<Vec<HeroicGameType>>,
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
            .join(".var/app/com.heroicgameslauncher.hgl/config/heroic/legendaryConfig/legendary/installed.json"),
        InstallationMode::UserBin => Path::new(&home_dir).join(".config/heroic/legendaryConfig/legendary/installed.json"),
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
) -> eyre::Result<Vec<HeroicGame>> {
    let installed_path = get_installed_json_location(install_mode);
    get_shortcuts_from_location(installed_path)
}

fn get_shortcuts_from_location<P: AsRef<Path>>(path: P) -> eyre::Result<Vec<HeroicGame>> {
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
    pub fn get_heroic_games(&self) -> eyre::Result<Vec<HeroicGameType>> {
        let install_modes = vec![InstallationMode::FlatPak, InstallationMode::UserBin];

        let mut heroic_games = self.get_epic_games(&install_modes)?;
        let gog_games = get_gog_games(&self.settings, &install_modes)?;
        heroic_games.extend(gog_games);
        Ok(heroic_games)
    }
}

impl NeedsProton<HeroicPlatform> for HeroicGameType {
    #[cfg(not(target_family = "unix"))]
    fn needs_proton(&self, _platform: &HeroicPlatform) -> bool {
        false
    }

    #[cfg(target_family = "unix")]
    fn needs_proton(&self, _platform: &HeroicPlatform) -> bool {
        match self {
            HeroicGameType::Epic(_game) => true,
            HeroicGameType::Gog(_, is_windows) => *is_windows,
            HeroicGameType::Heroic { .. } => false,
        }
    }

    fn create_symlinks(&self, _platform: &HeroicPlatform) -> bool {
        false
    }
}

impl HeroicPlatform {
    pub fn get_epic_games(
        &self,
        install_modes: &[InstallationMode],
    ) -> eyre::Result<Vec<HeroicGameType>> {
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
                    } else if game.is_installed() {
                        shortcuts.push(HeroicGameType::Epic(game));
                    }
                }
            }
        }
        Ok(shortcuts)
    }
}
fn get_gog_games(
    settings: &HeroicSettings,
    install_modes: &[InstallationMode],
) -> eyre::Result<Vec<HeroicGameType>> {
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

impl FromSettingsString for HeroicPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        HeroicPlatform {
            heroic_games: None,
            settings: load_settings(s),
        }
    }
}

impl GamesPlatform for HeroicPlatform {
    fn name(&self) -> &str {
        "Heroic"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts(self, self.get_heroic_games())
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Heroic");
        ui.checkbox(&mut self.settings.enabled, "Import from Heroic");
        ui.checkbox(
            &mut self.settings.default_launch_through_heroic,
            "Always launch games through Heroic",
        );
        let safe_mode_header = match (
            self.settings.default_launch_through_heroic,
            self.settings.launch_games_through_heroic.len(),
        ) {
            (false, 0) => "Force games to launch through Heroic Launcher".to_string(),
            (false, 1) => "One game forced to launch through Heroic Launcher".to_string(),
            (false, x) => format!("{x} games forced to launch through Heroic Launcher"),

            (true, 0) => "Force games to launch directly".to_string(),
            (true, 1) => "One game forced to launch directly".to_string(),
            (true, x) => format!("{x} games forced to launch directly"),
        };

        egui::CollapsingHeader::new(safe_mode_header).id_salt("Heroic_Launcher_safe_launch").show(ui, |ui| {
            if self.settings.default_launch_through_heroic{
                ui.label("Some games work best when launched directly, select those games below and BoilR will create shortcuts that launch the games directly.");
            } else {
                ui.label("Some games must be started from the Heroic Launcher, select those games below and BoilR will create shortcuts that opens the games through the Heroic Launcher.");
            }

            let manifests = self.heroic_games.get_or_insert_with(|| {
                let heroic_setting = self.settings.clone();
                let heroic_platform = HeroicPlatform{ settings:heroic_setting, heroic_games:None};
                heroic_platform.get_heroic_games().unwrap_or_default()
            });
            let safe_open_games = &mut self.settings.launch_games_through_heroic;

            for manifest in manifests{
                let key = manifest.app_name();
                let display_name = manifest.title();
                let mut safe_open = safe_open_games.contains(&display_name.to_string()) || safe_open_games.contains(&key.to_string());
                if ui.checkbox(&mut safe_open, display_name).clicked(){
                    if safe_open{
                        safe_open_games.push(key.to_string());
                    }else{
                        safe_open_games.retain(|m| m!= display_name && m!= key);
                    }
                }
            }
        });
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn code_name(&self) -> &str {
        "heroic"
    }
}

use serde::{Deserialize, Serialize};
use std::{
    env,
    path::{Path, PathBuf},
};
use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

use crate::platforms::{load_settings, to_shortcuts_simple, FromSettingsString, GamesPlatform};

use super::playnite_parser::parse_db;

impl FromSettingsString for PlaynitePlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        PlaynitePlatform {
            settings: load_settings(s),
        }
    }
}

#[derive(Clone)]
pub struct PlaynitePlatform {
    pub settings: PlayniteSettings,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PlayniteSettings {
    pub enabled: bool,
    pub installed_only: bool,
}

impl Default for PlayniteSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            installed_only: true,
        }
    }
}

impl GamesPlatform for PlaynitePlatform {
    fn name(&self) -> &str {
        "Playnite"
    }

    fn code_name(&self) -> &str {
        "playnite"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<crate::platforms::ShortcutToImport>> {
        to_shortcuts_simple(self.get_playnite_games())
    }

    fn get_settings_serilizable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Playnite");
        ui.checkbox(&mut self.settings.enabled, "Import from Playnite");
        if self.settings.enabled {
            ui.checkbox(
                &mut self.settings.installed_only,
                "Only import installed games",
            );
        }
    }
}

impl PlaynitePlatform {
    fn get_playnite_games(&self) -> eyre::Result<Vec<PlayniteGame>> {
        let mut res = vec![];
        let app_data_path = env::var("APPDATA")?;
        let app_data_local_path = env::var("LOCALAPPDATA")?;
        let launcher_path = Path::new(&app_data_local_path)
            .join("Playnite")
            .join("Playnite.DesktopApp.exe");
        if !launcher_path.exists() {
            return Err(eyre::eyre!("Did not find Playnite installation"));
        }
        let launcher_path = launcher_path.to_string_lossy().to_string();
        let playnite_folder = Path::new(&app_data_path).join("Playnite");
        let games_file_path = playnite_folder.join("library").join("games.db");
        if games_file_path.exists() {
            let games_bytes = std::fs::read(&games_file_path).unwrap();
            let (_, games) = parse_db(&games_bytes).map_err(|e| eyre::eyre!(e.to_string()))?;
            for game in games {
                if game.installed || !self.settings.installed_only {
                    res.push(PlayniteGame {
                        id: game.id,
                        launcher_path: launcher_path.clone().into(),
                        name: game.name,
                    });
                }
            }
        }
        Ok(res)
    }
}

impl From<PlayniteGame> for ShortcutOwned {
    fn from(game: PlayniteGame) -> Self {
        let launch = format!("--hidesplashscreen --start {}", game.id);
        let exe = game.launcher_path.to_string_lossy().to_string();
        let start_dir = game
            .launcher_path
            .parent()
            .unwrap_or_else(|| Path::new(""))
            .to_string_lossy()
            .to_string();
        Shortcut::new(
            "0",
            game.name.as_str(),
            exe.as_str(),
            start_dir.as_str(),
            "",
            "",
            launch.as_str(),
        )
        .to_owned()
    }
}

pub struct PlayniteGame {
    pub name: String,
    pub id: String,
    pub launcher_path: PathBuf,
}

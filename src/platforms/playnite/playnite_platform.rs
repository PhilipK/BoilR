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
    pub use_portalbe_version: bool,
    pub portable_launcher_path: String,
}

impl Default for PlayniteSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            installed_only: true,
            portable_launcher_path: String::new(),
            use_portalbe_version: false,
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

    fn get_settings_serializable(&self) -> String {
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
            ui.checkbox(
                &mut self.settings.use_portalbe_version,
                "Use Playnite Portable Version",
            );
            if self.settings.use_portalbe_version {
                ui.label("Path to portalbe Playnite.DesktopApp.exe file");
                ui.text_edit_singleline(&mut self.settings.portable_launcher_path);
            }
        }
    }
}

impl PlaynitePlatform {
    fn get_playnite_games(&self) -> eyre::Result<Vec<PlayniteGame>> {
        let mut res = vec![];
        let (launcher_path, games_file_path) = self.find_paths()?;
        let games_bytes = std::fs::read(games_file_path).map_err(|e| match e.raw_os_error() {
            Some(32) => {
                eyre::format_err!("It looks like Playnite is running and preventing BoilR from reading its database, please ensure that Playnite closed.")
            }
            _ => eyre::format_err!("Could not get Playnite games: {:?}", e),
        })?;
        let (_, games) = parse_db(&games_bytes).map_err(|e| eyre::eyre!(e.to_string()))?;
        for game in games {
            if game.installed || !self.settings.installed_only {
                res.push(PlayniteGame {
                    id: game.id,
                    launcher_path: launcher_path.clone(),
                    name: game.name,
                });
            }
        }
        Ok(res)
    }

    fn find_paths(&self) -> Result<(PathBuf, PathBuf), color_eyre::Report> {
        if self.settings.use_portalbe_version {
            let launcher_path = Path::new(&self.settings.portable_launcher_path).to_path_buf();
            let games_file_path = launcher_path
                .parent()
                .unwrap_or_else(|| Path::new(""))
                .join("library")
                .join("games.db");
            Ok((launcher_path, games_file_path))
        } else {
            let app_data_local_path = env::var("LOCALAPPDATA")?;
            let launcher_path = Path::new(&app_data_local_path)
                .join("Playnite")
                .join("Playnite.DesktopApp.exe");
            if !launcher_path.exists() {
                return Err(eyre::eyre!("Did not find Playnite installation"));
            }
            let app_data_path = env::var("APPDATA")?;
            let playnite_folder = Path::new(&app_data_path).join("Playnite");
            let games_file_path = playnite_folder.join("library").join("games.db");
            Ok((launcher_path, games_file_path))
        }
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

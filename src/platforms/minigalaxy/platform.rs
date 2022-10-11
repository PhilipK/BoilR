use serde::{Deserialize, Serialize};

use crate::platforms::{GamesPlatform, FromSettingsString, load_settings, GogShortcut, NeedsPorton};

#[derive(Clone)]
pub struct MiniGalaxyPlatform {
    settings: Settings,
}

#[derive(Deserialize, Serialize, Clone)]
struct Settings {
    enabled: bool,
    create_symlinks: bool,
}

impl Default for Settings{
    fn default() -> Self {
        #[cfg(target_family = "unix")]
        let enabled = true;
        #[cfg(target_family = "windows")]
        let enabled = false;
        Self { enabled, create_symlinks:false }
    }
}


impl NeedsPorton<MiniGalaxyPlatform> for GogShortcut{
    #[cfg(target_family = "unix")]
    fn needs_proton(&self, _platform: &MiniGalaxyPlatform) -> bool {
        //TODO check if we can do better than just always true
        //this might be a linux game
        true
    }

    #[cfg(not(target_family = "unix"))]
    fn needs_proton(&self, _platform: &MiniGalaxyPlatform) -> bool {
        false
    }

    #[cfg(not(target_family = "unix"))]
    fn create_symlinks(&self, _platform: &MiniGalaxyPlatform) -> bool {
        false
    }

    #[cfg(target_family = "unix")]
    fn create_symlinks(&self, platform: &MiniGalaxyPlatform) -> bool {
        platform.settings.create_symlinks
    }
}

impl FromSettingsString for MiniGalaxyPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        MiniGalaxyPlatform {
            settings: load_settings(s),
        }
    }
}

impl GamesPlatform for MiniGalaxyPlatform{
    fn name(&self) -> &str {
        "Mini Galaxy"
    }

    fn code_name(&self) -> &str {
        "minigalaxy"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<crate::platforms::ShortcutToImport>> {
        let home = std::env::var("HOME")?;
        let games_folder = std::path::Path::new(&home).join("GOG Games");
        let dirs = games_folder.read_dir()?;
        let mut game_folders = vec![];
        for game_folder in dirs{
            if let Ok(game_folder) = game_folder{
                game_folders.push(game_folder.path().to_owned());
            }
        }
        let gog_games = crate::platforms::get_gog_shortcuts_from_game_folders(game_folders);
        crate::platforms::to_shortcuts(self, Ok(gog_games))
    }

    fn get_settings_serilizable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mini Galaxy");
        ui.checkbox(&mut self.settings.enabled, "Import from Mini Galaxy");
        #[cfg(target_family = "unix")]
        if self.settings.enabled {
            ui.checkbox(&mut self.settings.create_symlinks, "Create symlinks");

        }
    }
}
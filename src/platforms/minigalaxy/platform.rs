use serde::{Deserialize, Serialize};

use crate::platforms::{GamesPlatform, FromSettingsString, load_settings};

#[derive(Clone)]
pub struct MiniGalaxyPlatform {
    settings: Settings,
}

#[derive(Deserialize, Serialize, Clone)]
struct Settings {
    enabled: bool,
}

impl Default for Settings{
    fn default() -> Self {
        #[cfg(target_family = "unix")]
        let enabled = true;
        #[cfg(target_family = "windows")]
        let enabled = false;
        Self { enabled }
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
        todo!()
    }

    fn get_settings_serilizable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Mini Galaxy");
        ui.checkbox(&mut self.settings.enabled, "Import from Mini Galaxy");
    }
}
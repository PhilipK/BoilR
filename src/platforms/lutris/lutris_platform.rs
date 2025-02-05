use super::game_list_parser::parse_lutris_games;
use super::lutris_game::LutrisGame;
use super::settings::LutrisSettings;
use crate::platforms::{
    load_settings, to_shortcuts_simple, FromSettingsString, GamesPlatform, ShortcutToImport,
};
use std::process::Command;

#[derive(Clone)]
pub struct LutrisPlatform {
    pub settings: LutrisSettings,
}

impl LutrisPlatform {
    fn get_shortcuts(&self) -> eyre::Result<Vec<LutrisGame>> {
        let output = get_lutris_command_output(&self.settings)?;
        let games = parse_lutris_games(output.as_str());
        let installed = self.settings.installed;
        let mut res = vec![];
        for mut game in games {
            let service = if installed { game.runner.clone().unwrap_or_default() } else { game.service.clone().unwrap_or_default() };
            if service != "steam" {
                game.settings = Some(self.settings.clone());
                res.push(game);
            }
        }
        Ok(res)
    }
}

fn get_lutris_command_output(settings: &LutrisSettings) -> eyre::Result<String> {
    let output = if settings.flatpak {
        let flatpak_image = &settings.flatpak_image;
        #[cfg(not(feature = "flatpak"))]
        {
            let mut command = Command::new("flatpak");
            command.arg("run").arg(flatpak_image).arg("--json");
            if settings.installed {
                command.arg("-lo").output()?
            } else {
                command.arg("-a").output()?
            }
        }
        #[cfg(feature = "flatpak")]
        {
            let mut command = Command::new("flatpak-spawn");
            command
                .arg("--host")
                .arg("flatpak")
                .arg("run")
                .arg(flatpak_image);
            command.arg("run").arg(flatpak_image).arg("--json");
            if settings.installed {
                command.arg("-lo").output()?
            } else {
                command.arg("-a").output()?
            }
        }
    } else {
        let mut command = Command::new(&settings.executable);
        command.arg("--json");
        if settings.installed {
            command.arg("-lo").output()?
        } else {
            command.arg("-a").output()?
        }
    };

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

impl FromSettingsString for LutrisPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        LutrisPlatform {
            settings: load_settings(s),
        }
    }
}

impl GamesPlatform for LutrisPlatform {
    fn name(&self) -> &str {
        "Lutris"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts_simple(self.get_shortcuts())
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Lutris");
        ui.checkbox(&mut self.settings.enabled, "Import from Lutris");
        if self.settings.enabled {
            ui.checkbox(&mut self.settings.installed, "Search installed only");
            ui.checkbox(&mut self.settings.flatpak, "Flatpak version");
            if !self.settings.flatpak {
                ui.horizontal(|ui| {
                    let lutris_location = &mut self.settings.executable;
                    ui.label("Lutris Location: ");
                    ui.text_edit_singleline(lutris_location);
                });
            } else {
                ui.horizontal(|ui| {
                    let flatpak_image = &mut self.settings.flatpak_image;
                    ui.label("Flatpak image");
                    ui.text_edit_singleline(flatpak_image);
                });
            }
        }
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn code_name(&self) -> &str {
        "lutris"
    }
}

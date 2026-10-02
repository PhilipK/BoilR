use super::legendary_game::LegendaryGame;
use super::LegendarySettings;
use crate::platforms::{
    load_settings, to_shortcuts_simple, FromSettingsString, GamesPlatform, ShortcutToImport,
};
use serde_json::from_str;
use std::process::Command;

#[derive(Clone)]
pub struct LegendaryPlatform {
    pub settings: LegendarySettings,
}

impl LegendaryPlatform {
    fn get_shortcuts(&self) -> eyre::Result<Vec<LegendaryGame>> {
        let legendary_string = self
            .settings
            .executable
            .clone()
            .unwrap_or_else(|| "legendary".to_string());
        let legendary = legendary_string.as_str();
        execute_legendary_command(legendary)
    }
}

fn execute_legendary_command(program: &str) -> eyre::Result<Vec<LegendaryGame>> {
    let legendary_command = Command::new(program)
        .arg("list-installed")
        .arg("--json")
        .output()?;
    let json = String::from_utf8_lossy(&legendary_command.stdout);
    parse_installed_games(&json)
}

/// `legendary list-installed` lists installed DLC next to the games. A DLC can't be
/// launched on its own, so it must not become a shortcut (#557).
fn parse_installed_games(json: &str) -> eyre::Result<Vec<LegendaryGame>> {
    let games: Vec<LegendaryGame> = from_str(json)?;
    Ok(games.into_iter().filter(|game| !game.is_dlc).collect())
}

impl GamesPlatform for LegendaryPlatform {
    fn name(&self) -> &str {
        "Legendary"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts_simple(self.get_shortcuts())
    }

    #[cfg(feature = "egui-ui")]
    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Legendary & Rare");
        ui.checkbox(&mut self.settings.enabled, "Import from Legendary & Rare");
        if self.settings.enabled {
            ui.horizontal(|ui| {
                let mut empty_string = "".to_string();
                let legendary_location = self
                    .settings
                    .executable
                    .as_mut()
                    .unwrap_or(&mut empty_string);
                ui.label("Legendary Executable: ")
                    .on_hover_text("The location of the legendary executable to use");
                if ui.text_edit_singleline(legendary_location).changed() {
                    self.settings.executable = Some(legendary_location.to_string());
                }
            });
        }
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn code_name(&self) -> &str {
        "legendary"
    }
}

impl FromSettingsString for LegendaryPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        LegendaryPlatform {
            settings: load_settings(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_installed_games_skips_dlc() -> eyre::Result<()> {
        let json = r#"[
            {
                "app_name": "Eider",
                "can_run_offline": true,
                "title": "HITMAN World of Assassination",
                "is_dlc": false,
                "install_path": "/home/deck/Games/HITMAN3",
                "executable": "Launcher.exe"
            },
            {
                "app_name": "0a73eaedcac84bd28b567dbec764c5cb",
                "can_run_offline": true,
                "title": "HITMAN 3 - Seven Deadly Sins Collection",
                "is_dlc": true,
                "install_path": "/home/deck/Games/HITMAN3",
                "executable": ""
            }
        ]"#;

        let games = parse_installed_games(json)?;
        let [game] = games.as_slice() else {
            eyre::bail!("expected exactly one game, got {}", games.len());
        };
        assert_eq!(game.app_name, "Eider");
        Ok(())
    }
}

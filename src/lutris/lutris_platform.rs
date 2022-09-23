use super::game_list_parser::parse_lutris_games;
use super::lutris_game::LutrisGame;
use super::settings::LutrisSettings;
use crate::platforms::{Platform, SettingsValidity};
use std::error::Error;
use std::process::Command;

pub struct LutrisPlatform {
    pub settings: LutrisSettings,
}

impl Platform<LutrisGame, Box<dyn Error>> for LutrisPlatform {
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn name(&self) -> &str {
        "Lutris"
    }

    fn get_shortcuts(&self) -> Result<Vec<LutrisGame>, Box<dyn Error>> {
        let output = get_lutris_command_output(&self.settings)?;
        let games = parse_lutris_games(output.as_str());
        let mut res = vec![];
        for mut game in games {
            if game.runner != "steam" {
                game.settings = Some(self.settings.clone());
                res.push(game);
            }
        }
        Ok(res)
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

    fn needs_proton(&self, _input: &LutrisGame) -> bool {
        false
    }
}

fn get_lutris_command_output(settings: &LutrisSettings) -> Result<String, Box<dyn Error>> {
    let output = if settings.flatpak {
        let flatpak_image = &settings.flatpak_image;
        #[cfg(not(feature = "flatpak"))]{
            let mut command = Command::new("flatpak");
            command.arg("run").arg(flatpak_image).arg("-lo").arg("--json").output()?
        }
        #[cfg(feature = "flatpak")]
        {
            let mut command = Command::new("flatpak-spawn");
            command.arg("--host").arg("flatpak").arg("run").arg(flatpak_image).arg("-lo").arg("--json").output()?
        }
    } else {
        let mut command = Command::new(&settings.executable);
        command.arg("-lo").arg("--json").output()?
    };

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

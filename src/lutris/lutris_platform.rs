use super::game_list_parser::parse_lutris_games;
use super::lutris_game::LutrisGame;
use super::settings::LutrisSettings;
use crate::platform::{Platform, SettingsValidity};
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
        let default_lutris_exe = "lutris".to_string();
        let lutris_executable = self.settings.executable.as_ref().unwrap_or(&default_lutris_exe);
        let lutris_command = Command::new(lutris_executable).arg("-lo").output()?;
        let output = String::from_utf8_lossy(&lutris_command.stdout).to_string();
        let  games = parse_lutris_games(output.as_str());
        let mut res = vec![];
        for game in games {
            if game.platform != "steam" {
                res.push(game);
            }
        }
        Ok(res)
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
}

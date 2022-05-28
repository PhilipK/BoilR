use serde::{Deserialize, Serialize};

use crate::platform::{Platform, SettingsValidity};
use std::error::Error;

use super::FlatpakSettings;
use steam_shortcuts_util::{shortcut::ShortcutOwned, Shortcut};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct FlatpakPlatform {
    pub settings: FlatpakSettings,
}

#[derive(Debug, Clone)]
pub struct FlatpakApp {
    pub name: String,
    pub id: String,
}

impl From<FlatpakApp> for ShortcutOwned {
    fn from(app: FlatpakApp) -> Self {
        let launch_parameter = format!("run {}", app.id);
        Shortcut::new("0", &app.name, "flatpak", "", "", "", &launch_parameter).to_owned()
    }
}

impl Platform<FlatpakApp, Box<dyn Error>> for FlatpakPlatform {
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn name(&self) -> &str {
        "Flatpak"
    }

    fn get_shortcuts(&self) -> Result<Vec<FlatpakApp>, Box<dyn Error>> {
        use std::process::Command;
        let mut command = Command::new("flatpak");
        let output = command
            .arg("list")
            .arg("--app")
            .arg("--columns=name,application")
            .output()?;
        let output_string = String::from_utf8_lossy(&output.stdout).to_string();
        let mut result = vec![];
        for line in output_string.lines() {
            let mut split = line.split("\t");
            if let Some(name) = split.next() {
                if let Some(id) = split.next() {
                    result.push(FlatpakApp {
                        name: name.to_string(),
                        id: id.to_string(),
                    })
                }
            }
        }
        Ok(result)
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

    #[cfg(target_family = "unix")]
    fn create_symlinks(&self) -> bool {
        false
    }

    fn needs_proton(&self, _input: &FlatpakApp) -> bool {
        false
    }
}

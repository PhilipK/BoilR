use super::{HeroicGame, HeroicSettings};
use crate::platform::{Platform, SettingsValidity};
use serde_json::from_str;
use std::error::Error;
use std::path::Path;
use std::process::Command;

pub struct HeroicPlatform {
   pub settings: HeroicSettings,
}

impl HeroicPlatform {
    pub fn new(settings: HeroicSettings) -> HeroicPlatform {
        Self { settings }
    }
}

impl Platform<HeroicGame, Box<dyn Error>> for HeroicPlatform {
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn name(&self) -> &str {
        "Heroic"
    }

    fn get_shortcuts(&self) -> Result<Vec<HeroicGame>, Box<dyn Error>> {
        let legendary = "/var/lib/flatpak/app/com.heroicgameslauncher.hgl/current/active/files/bin/heroic/resources/app.asar.unpacked/build/bin/linux/";
        let home_dir = std::env::var("HOME").unwrap_or("".to_string());
        let home = Path::new(&home_dir);
        let config_folder = home.join("/.var/app/com.heroicgameslauncher.hgl/config");
        let legendary_command = Command::new(legendary)
            .arg("list-installed")
            .arg("--json")
            .env("XDG_CONFIG_HOME", config_folder)
            .output()?;
        let json = String::from_utf8_lossy(&legendary_command.stdout);
        let legendary_ouput = from_str(&json)?;
        
        Ok(legendary_ouput)
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

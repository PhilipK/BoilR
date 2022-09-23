use std::{
    error::Error,
    path::{Path, PathBuf},
};

use sqlite::State;
use steam_shortcuts_util::shortcut::ShortcutOwned;

use crate::platform::Platform;

use super::{AmazonGame, AmazonSettings};

#[derive(Clone)]
pub struct AmazonPlatform {
    pub settings: AmazonSettings,
}

impl Platform<AmazonGame, Box<dyn Error>> for AmazonPlatform {
    #[cfg(windows)]
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    #[cfg(target_family = "unix")]
    fn enabled(&self) -> bool {
        false
    }

    fn name(&self) -> &str {
        "Amazon"
    }

    fn get_shortcuts(&self) -> Result<Vec<AmazonGame>, Box<dyn Error>> {
        let sqllite_path = get_sqlite_path()?;
        let launcher_path = get_launcher_path()?;
        let mut result = vec![];
        let connection = sqlite::open(sqllite_path)?;
        let mut statement =
            connection.prepare("SELECT Id, ProductTitle FROM DbSet WHERE Installed = 1")?;
        while let State::Row = statement.next().unwrap() {
            let id = statement.read::<String>(0);
            let title = statement.read::<String>(1);
            if let (Ok(id), Ok(title)) = (id, title) {
                result.push(AmazonGame {
                    title,
                    id,
                    launcher_path: launcher_path.clone(),
                });
            }
        }
        Ok(result)
    }

    fn settings_valid(&self) -> crate::platform::SettingsValidity {
        let path = get_sqlite_path();
        let launcher = get_launcher_path();
        if path.is_ok() && launcher.is_ok() {
            crate::platform::SettingsValidity::Valid
        } else {
            crate::platform::SettingsValidity::Invalid {
                reason: "Could not find Amazon Games installation".to_string(),
            }
        }
    }

    #[cfg(target_family = "unix")]
    fn create_symlinks(&self) -> bool {
        false
    }

    fn needs_proton(&self, _input: &AmazonGame) -> bool {
        false
    }
}

fn get_sqlite_path() -> eyre::Result<PathBuf> {
    let localdata = std::env::var("LOCALAPPDATA")?;
    let path = Path::new(&localdata)
        .join("Amazon Games")
        .join("Data")
        .join("Games")
        .join("Sql")
        .join("GameInstallInfo.sqlite");
    if path.exists() {
        Ok(path)
    } else {
        Err(eyre::format_err!(
            "Amazong GameInstallInfo.sqlite not found at {:?}",path
        ))
    }
}

fn get_launcher_path() -> eyre::Result<PathBuf> {
    let localdata = std::env::var("LOCALAPPDATA")?;
    let path = Path::new(&localdata)
        .join("Amazon Games")
        .join("App")
        .join("Amazon Games.exe");
    if path.exists() {
        Ok(path)
    } else {
        Err(eyre::format_err!(
            "Could not find Amazon Games.exe at {:?}",path
        ))
    }
}

impl AmazonPlatform {
    pub fn render_amazon_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Amazon");
        ui.checkbox(&mut self.settings.enabled, "Import from Amazon");
    }

    pub fn get_owned_shortcuts(&self) -> Result<Vec<ShortcutOwned>, String> {
        self.get_shortcuts()
            .map(|shortcuts| shortcuts.iter().map(|m| m.clone().into()).collect())
            .map_err(|err| format!("{:?}", err))
    }
}

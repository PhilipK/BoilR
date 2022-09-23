use std::path::{Path, PathBuf};

use sqlite::State;

use super::{AmazonGame, AmazonSettings};

#[derive(Clone)]
pub struct AmazonPlatform {
    pub settings: AmazonSettings,
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
            "Amazong GameInstallInfo.sqlite not found at {:?}",
            path
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
            "Could not find Amazon Games.exe at {:?}",
            path
        ))
    }
}

impl AmazonPlatform {
    pub fn get_amazon_games(&self) -> eyre::Result<Vec<AmazonGame>> {
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

    pub fn render_amazon_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Amazon");
        ui.checkbox(&mut self.settings.enabled, "Import from Amazon");
    }
}

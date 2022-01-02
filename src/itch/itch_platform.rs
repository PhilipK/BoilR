use super::butler_db_parser::*;
use super::receipt::Receipt;
use super::{ItchGame, ItchSettings};
use crate::platform::{Platform, SettingsValidity};
use failure::*;
use flate2::read::GzDecoder;
use std::collections::HashSet;
use std::io::prelude::*;
use std::path::Path;

pub struct ItchPlatform {
    settings: ItchSettings,
}

impl ItchPlatform {
    pub fn new(settings: ItchSettings) -> ItchPlatform {
        ItchPlatform { settings }
    }
}

impl Platform<ItchGame, ItchErrors> for ItchPlatform {
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn name(&self) -> &str {
        "Itch"
    }

    fn get_shortcuts(&self) -> Result<Vec<ItchGame>, ItchErrors> {
        let itch_location = self.settings.location.clone();
        let itch_location = itch_location.unwrap_or_else(get_default_location);

        let itch_db_location = Path::new(&itch_location).join("db").join("butler.db-wal");
        if !itch_db_location.exists() {
            return Err(ItchErrors::PathNotFound {
                path: itch_db_location.to_str().unwrap().to_string(),
            });
        }

        let shortcut_bytes = std::fs::read(&itch_db_location).unwrap();

        let paths = match parse_butler_db(&shortcut_bytes) {
            Ok((_, shortcuts)) => Ok(shortcuts),
            Err(e) => Err(ItchErrors::ParseError {
                error: e.to_string(),
                path: itch_db_location.to_str().unwrap().to_string(),
            }),
        }?;

        //This is done to remove douplicates
        let paths: HashSet<&DbPaths> = paths.iter().collect();

        let res = paths.iter().filter_map(|e| dbpath_to_game(*e)).collect();
        Ok(res)
    }

    #[cfg(target_os = "linux")]
    fn create_symlinks(&self) -> bool {
        self.settings.create_symlinks
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

fn dbpath_to_game(paths: &DbPaths<'_>) -> Option<ItchGame> {
    let recipt = Path::new(paths.base_path)
        .join(".itch")
        .join("receipt.json.gz");
    if !&recipt.exists() {
        return None;
    }

    let gz_bytes = std::fs::read(&recipt).unwrap();
    let mut d = GzDecoder::new(gz_bytes.as_slice());
    let mut s = String::new();
    d.read_to_string(&mut s).unwrap();

    let receipt_op: Option<Receipt> = serde_json::from_str(&s).ok();
    receipt_op.map(|re| ItchGame {
        install_path: paths.base_path.to_owned(),
        executable: paths.path.to_owned(),
        title: re.game.title,
    })
}

#[cfg(target_os = "linux")]
pub fn get_default_location() -> String {
    //If we don't have a home drive we have to just die
    let home = std::env::var("HOME").expect("Expected a home variable to be defined");
    format!("{}/.config/itch/", home)
}

#[cfg(target_os = "windows")]
pub fn get_default_location() -> String {
    let key = "APPDATA";
    let appdata = std::env::var(key).expect("Expected a APPDATA variable to be defined");
    Path::new(&appdata)
        .join("itch")
        .to_str()
        .unwrap()
        .to_string()
    //C:\Users\phili\AppData\Local\itch
}

#[derive(Debug, Fail)]
pub enum ItchErrors {
    #[fail(
        display = "Itch path: {} could not be found. Try to specify a different path for the Itch.",
        path
    )]
    PathNotFound { path: String },

    #[fail(display = "Could not parse Itch db at {} error: {}", path, error)]
    ParseError { path: String, error: String },
}

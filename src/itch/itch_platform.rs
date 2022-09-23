use super::butler_db_parser::*;
use super::receipt::Receipt;
use super::{ItchGame, ItchSettings};
use crate::platforms::{Platform, SettingsValidity};
use flate2::read::GzDecoder;
use is_executable::IsExecutable;
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

impl Platform<ItchGame, String> for ItchPlatform {
    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn name(&self) -> &str {
        "Itch"
    }

    fn get_shortcuts(&self) -> Result<Vec<ItchGame>, String> {
        let itch_location = self.settings.location.clone();
        let itch_location = itch_location.unwrap_or_else(get_default_location);

        let itch_db_location = Path::new(&itch_location).join("db").join("butler.db-wal");
        if !itch_db_location.exists() {
            return Err(format!("Path not found: {:?}", itch_db_location.to_str()));
        }

        let shortcut_bytes = std::fs::read(&itch_db_location).unwrap();

        let paths = match parse_butler_db(&shortcut_bytes) {
            Ok((_, shortcuts)) => Ok(shortcuts),
            Err(e) => Err(format!(
                "Could not parse path: {:?} , error: {:?}",
                itch_db_location.to_str(),
                e
            )),
        }?;

        //This is done to paths dedupe
        let paths: HashSet<&DbPaths> = paths.iter().collect();
        let res = paths.iter().filter_map(|e| dbpath_to_game(*e)).collect();
        Ok(res)
    }

    #[cfg(target_family = "unix")]
    fn create_symlinks(&self) -> bool {
        self.settings.create_symlinks
    }

    fn settings_valid(&self) -> crate::platforms::SettingsValidity {
        let shortcuts_res = self.get_shortcuts();
        match shortcuts_res {
            Ok(_) => SettingsValidity::Valid,
            Err(err) => SettingsValidity::Invalid {
                reason: err.to_string(),
            },
        }
    }
    #[cfg(target_os = "windows")]
    fn needs_proton(&self, _input: &ItchGame) -> bool {
        false
    }

    #[cfg(target_family = "unix")]
    fn needs_proton(&self, input: &ItchGame) -> bool {
        //We can only really guess here
        input.executable.ends_with("exe")
    }
}

fn dbpath_to_game(paths: &DbPaths) -> Option<ItchGame> {
    let recipt = Path::new(paths.base_path.as_str())
        .join(".itch")
        .join("receipt.json.gz");
    if !&recipt.exists() {
        return None;
    }

    let executable = paths
        .paths
        .iter()
        .find(|p| Path::new(&paths.base_path).join(&p).is_executable());
    match executable {
        Some(executable) => {
            let gz_bytes = std::fs::read(&recipt).unwrap();
            let mut d = GzDecoder::new(gz_bytes.as_slice());
            let mut s = String::new();
            d.read_to_string(&mut s).unwrap();

            let receipt_op: Option<Receipt> = serde_json::from_str(&s).ok();
            receipt_op.map(|re| ItchGame {
                install_path: paths.base_path.to_owned(),
                executable: executable.to_owned(),
                title: re.game.title,
            })
        }
        None => None,
    }
}

#[cfg(target_family = "unix")]
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

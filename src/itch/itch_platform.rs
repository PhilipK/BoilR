use super::butler_db_parser::*;
use super::{ItchGame, ItchSettings};
use crate::platform::Platform;
use failure::*;
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

        let res = match parse_butler_db(&shortcut_bytes) {
            Ok((_, shortcuts)) => Ok(shortcuts),
            Err(e) => Err(ItchErrors::ParseError {
                error: e.to_string(),
                path: itch_db_location.to_str().unwrap().to_string(),
            }),
        }?;

        let res = res
            .iter()
            .map(|paths| ItchGame {
                install_path: paths.base_path.to_owned(),
                executable: paths.path.to_owned(),
                title: paths.path.to_owned(),
            })
            .collect();
        Ok(res)
    }
}

#[cfg(target_os = "linux")]
fn get_default_location() -> String {
    //If we don't have a home drive we have to just die
    let home = std::env::var("HOME").expect("Expected a home variable to be defined");
    format!("{}/.config/itch/", home)
}

#[cfg(target_os = "windows")]
fn get_default_location() -> String {
    let key = "PROGRAMFILES(X86)";
    let program_files = env::var(key).expect("Expected a program files variable to be defined");
    format!("{}//Itch//", program_files)
}

#[derive(Debug, Fail)]
pub enum ItchErrors {
    #[fail(
        display = "Itch path: {} could not be found. Try to specify a different path for the Itch.",
        path
    )]
    PathNotFound { path: String },

    #[fail(display = "Could not read Itch db at {} error: {}", path, error)]
    ReadDirError { path: String, error: std::io::Error },

    #[fail(display = "Could not parse Itch db at {} error: {}", path, error)]
    ParseError { path: String, error: String },
}

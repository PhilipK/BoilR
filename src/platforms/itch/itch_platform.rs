use super::butler_db_parser::*;
use super::itch_game::ItchGame;
use super::receipt::Receipt;
use super::ItchSettings;
use crate::platforms::{
    load_settings, to_shortcuts, FromSettingsString, GamesPlatform, ShortcutToImport,
};
use flate2::read::GzDecoder;
use is_executable::IsExecutable;
use std::collections::HashSet;
use std::io::prelude::*;
use std::path::Path;

#[derive(Clone)]
pub struct ItchPlatform {
    pub settings: ItchSettings,
}

impl ItchPlatform {
    fn get_itch_games(&self) -> eyre::Result<Vec<ItchGame>> {
        let itch_location = self.settings.location.clone();
        let itch_location = itch_location.unwrap_or_else(get_default_location);

        let itch_db_location = Path::new(&itch_location).join("db").join("butler.db-wal");
        if !itch_db_location.exists() {
            return Err(eyre::format_err!(
                "Path not found: {:?}",
                itch_db_location.to_str()
            ));
        }

        let shortcut_bytes = std::fs::read(&itch_db_location)?;

        let paths = match parse_butler_db(&shortcut_bytes) {
            Ok((_, shortcuts)) => Ok(shortcuts),
            Err(e) => Err(eyre::format_err!(
                "Could not parse path: {:?} , error: {:?}",
                itch_db_location.to_str(),
                e
            )),
        }?;

        //This is done to paths dedupe
        let paths: HashSet<&DbPaths> = paths.iter().collect();
        let res = paths.iter().filter_map(|e| dbpath_to_game(e)).collect();
        Ok(res)
    }
}

fn dbpath_to_game(paths: &DbPaths) -> Option<ItchGame> {
    let recipt = Path::new(paths.base_path.as_str())
        .join(".itch")
        .join("receipt.json.gz");
    if !&recipt.exists() {
        return None;
    }
    paths
        .paths
        .iter()
        .filter(|p| Path::new(&paths.base_path).join(p).is_executable())
        .find_map(|executable| {
            if let Ok(gz_bytes) = std::fs::read(&recipt) {
                let mut d = GzDecoder::new(gz_bytes.as_slice());
                let mut s = String::new();
                if d.read_to_string(&mut s).is_ok() {
                    let receipt_op: Option<Receipt> = serde_json::from_str(&s).ok();
                    return receipt_op.map(|re| ItchGame {
                        install_path: paths.base_path.to_owned(),
                        executable: executable.to_owned(),
                        title: re.game.title,
                    });
                }
            }
            None
        })
}

#[cfg(target_family = "unix")]
pub fn get_default_location() -> String {
    //If we don't have a home drive we have to just die
    let home = std::env::var("HOME").unwrap_or_default();
    format!("{home}/.config/itch/")
}

#[cfg(target_os = "windows")]
pub fn get_default_location() -> String {
    let key = "APPDATA";
    std::env::var(key)
        .map(|appdata| {
            Path::new(&appdata)
                .join("itch")
                .to_string_lossy()
                .to_string()
        })
        .unwrap_or_default()
    //C:\Users\phili\AppData\Local\itch
}

impl FromSettingsString for ItchPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        ItchPlatform {
            settings: load_settings(s),
        }
    }
}

impl GamesPlatform for ItchPlatform {
    fn name(&self) -> &str {
        "Itch"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<ShortcutToImport>> {
        to_shortcuts(self, self.get_itch_games())
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Itch.io");
        ui.checkbox(&mut self.settings.enabled, "Import from Itch.io");
        if self.settings.enabled {
            ui.horizontal(|ui| {
                let mut empty_string = "".to_string();
                let itch_location = self.settings.location.as_mut().unwrap_or(&mut empty_string);
                ui.label("Itch.io Folder: ");
                if ui.text_edit_singleline(itch_location).changed() {
                    self.settings.location = if itch_location.is_empty() {
                        None
                    } else {
                        Some(itch_location.to_string())
                    };
                } else if !itch_location.is_empty()
                    && ui
                        .button("Reset")
                        .on_hover_text("Reset the itch path, let BoilR guess again")
                        .clicked()
                {
                    self.settings.location = None;
                }
            });
            #[cfg(target_family = "unix")]
            {
                ui.checkbox(&mut self.settings.create_symlinks, "Create symlinks");
            }
        }
    }

    fn get_settings_serializable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn code_name(&self) -> &str {
        "itch"
    }
}

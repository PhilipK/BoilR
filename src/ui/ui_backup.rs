use std::{
    path::{Path, PathBuf},
    time::SystemTime,
};

use chrono::Local;

use crate::{
    config::get_backups_flder,
    steam::{get_shortcuts_paths, SteamSettings},
};

use super::MyEguiApp;

#[derive(Default)]
pub struct BackupState {
    pub available_backups: Option<Vec<PathBuf>>,
}

impl MyEguiApp {
    pub fn render_backup(&mut self, ui: &mut egui::Ui) {
        ui.heading("Backups");
        ui.label("Here you can restore shortcuts from bakcups");

        let available_backups = self
            .backup_state
            .available_backups
            .get_or_insert_with(|| load_backups());

        if available_backups.is_empty() {
            ui.label(
                "No backed up shortcuts found, they will be created every time you run import",
            );
        }
        for backup_path in available_backups {
            if ui
                .button(backup_path.to_string_lossy().to_string())
                .clicked()
            {
                //Restore
                backup_shortcuts(&self.settings.steam);
                restore_backup(&self.settings.steam, backup_path.as_path());
            }
        }

        if ui.button("Back up shortcuts").clicked() {
            backup_shortcuts(&self.settings.steam);
            self.backup_state.available_backups = None;
        }
    }
}

pub fn restore_backup(steam_settings: &SteamSettings, shortcut_path: &Path) {
    let file_name = shortcut_path.file_name().unwrap();
    let paths = get_shortcuts_paths(steam_settings);
    if let Ok(paths) = paths {
        for user in paths {
            if let Some(user_shortcut_path) = user.shortcut_path {
                if file_name.to_string_lossy().starts_with(&user.user_id) {
                    std::fs::copy(shortcut_path, Path::new(&user_shortcut_path)).unwrap();
                    println!("Restored shortcut to path : {}", user_shortcut_path);
                }
            }
        }
    }
}

pub fn load_backups() -> Vec<PathBuf> {
    let backup_folder = get_backups_flder();
    let files = std::fs::read_dir(&backup_folder);
    let mut result = vec![];
    if let Ok(files) = files {
        for file in files {
            if let Ok(file) = file {
                if file
                    .path()
                    .extension()
                    .unwrap_or_default()
                    .to_string_lossy()
                    == "vdf"
                {
                    result.push(file.path().to_path_buf());
                }
            }
        }
    }
    return result;
}

pub fn backup_shortcuts(steam_settings: &SteamSettings) {
    let backup_folder = get_backups_flder();
    let paths = get_shortcuts_paths(&steam_settings);
    let date = Local::now();
    let date_string = date.format("%Y-%m-%d-%H-%M-%S");
    if let Ok(user_infos) = paths {
        for user_info in user_infos {
            if let Some(shortcut_path) = user_info.shortcut_path {
                let new_path = backup_folder.join(format!(
                    "{}-{}-shortcuts.vdf",
                    user_info.user_id, date_string
                ));
                println!("Backed up shortcut at: {:?}", new_path);
                std::fs::copy(&shortcut_path, &new_path).unwrap();
            }
        }
    }
}

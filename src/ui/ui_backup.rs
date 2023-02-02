use std::path::{Path, PathBuf};

use egui::ScrollArea;
use time::format_description;

use crate::{
    config::get_backups_flder,
    steam::{get_shortcuts_paths, SteamSettings},
};

use super::MyEguiApp;

#[derive(Default)]
pub struct BackupState {
    pub available_backups: Option<Vec<PathBuf>>,
    pub last_restore: Option<PathBuf>,
}

impl MyEguiApp {
    pub fn render_backup(&mut self, ui: &mut egui::Ui) {
        ui.heading("Backups");
        ui.label("Here you can restore backed up shortcuts files");
        ui.label("Click a backup to restore it, your current shortcuts will be backed up first");
        ui.add_space(15.0);

        if let Some(last_restore) = self.backup_state.last_restore.as_ref() {
            ui.heading(format!("Last restored {last_restore:?}"));
        }

        if ui.button("Click here to create a new backup").clicked() {
            backup_shortcuts(&self.settings.steam);
            self.backup_state.available_backups = None;
        }

        let available_backups = self
            .backup_state
            .available_backups
            .get_or_insert_with(load_backups);

        if available_backups.is_empty() {
            ui.label("No backups found, they will be created every time you run import");
        } else {
            ScrollArea::vertical()
                .stick_to_right(true)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for backup_path in available_backups {
                        if ui
                            .button(backup_path.to_string_lossy().to_string())
                            .clicked()
                        {
                            //Restore
                            backup_shortcuts(&self.settings.steam);
                            if restore_backup(&self.settings.steam, backup_path.as_path()) {
                                self.backup_state.last_restore = Some(backup_path.clone());
                            }
                        }
                    }
                });
        }
    }
}

pub fn restore_backup(steam_settings: &SteamSettings, shortcut_path: &Path) -> bool {
    let file_name = shortcut_path.file_name();
    let paths = get_shortcuts_paths(steam_settings);
    if let (Ok(paths), Some(file_name)) = (paths, file_name) {
        for user in paths {
            if let Some(user_shortcut_path) = user.shortcut_path {
                if file_name.to_string_lossy().starts_with(&user.user_id) {
                    match std::fs::copy(shortcut_path, Path::new(&user_shortcut_path)) {
                        Ok(_) => {
                            println!("Restored shortcut to path : {user_shortcut_path}");
                        }
                        Err(err) => {
                            eprintln!(
                                "Failed to restored shortcut to path : {user_shortcut_path} gave error: {err:?}"
                            );
                        }
                    }
                    return true;
                }
            }
        }
    }
    false
}

pub fn load_backups() -> Vec<PathBuf> {
    let backup_folder = get_backups_flder();
    let files = std::fs::read_dir(backup_folder);
    let mut result = vec![];
    if let Ok(files) = files {
        for file in files.flatten() {
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
    result.sort();
    result.reverse();
    result
}

const DATE_FORMAT: &str = "[year]-[month]-[day]-[hour]-[minute]-[second]";

pub fn backup_shortcuts(steam_settings: &SteamSettings) {
    use time::OffsetDateTime;

    let backup_folder = get_backups_flder();
    let paths = get_shortcuts_paths(steam_settings);
    let date = OffsetDateTime::now_utc();
    let format = format_description::parse(DATE_FORMAT);
    if let Ok(format) = format{
    let date_string = date.format(&format);
    if let (Ok(date_string),Ok(user_infos)) = (date_string,paths) {
        for user_info in user_infos {
            if let Some(shortcut_path) = user_info.shortcut_path {
                let new_path = backup_folder.join(format!(
                    "{}-{}-shortcuts.vdf",
                    user_info.user_id, date_string
                ));
                match std::fs::copy(shortcut_path, &new_path) {
                    Ok(_) => {
                        println!("Backed up shortcut at: {new_path:?}");
                    }
                    Err(err) => {
                        eprintln!(
                            "Failed to backup shortcut at: {new_path:?}, error: {err:?}"
                        );
                    }
                }
            }
        }}
    }
}

use std::{path::PathBuf, time::SystemTime};

use chrono::Local;

use crate::{config::get_backups_flder, steam::{get_shortcuts_paths, SteamSettings}};

use super::MyEguiApp;

#[derive(Default)]
pub struct BackupState{
    pub available_backups: Option<Vec<PathBuf>>,
}

impl MyEguiApp {
    pub fn render_backup(&mut self, ui: &mut egui::Ui) {
        ui.heading("Backups");

        let available_backups = self.backup_state.available_backups.get_or_insert_with(||{
            load_backups()
        });

        for backup_path in available_backups{
            if ui.button(backup_path.to_string_lossy().to_string()).clicked(){
                //Restore
            }
        }


        if ui.button("Back up shortcuts").clicked() {
            backup_shortcuts(&self.settings.steam);
            self.backup_state.available_backups = None;
        }
    }
}

pub fn load_backups() -> Vec<PathBuf>{
    let backup_folder = get_backups_flder();
    let files = std::fs::read_dir(&backup_folder);
    let mut result = vec![];
    if let Ok(files) = files{
        for file in files {
            if let Ok(file) = file{
                if file.path().extension().unwrap_or_default().to_string_lossy() == "vdf" {
                    result.push(file.path().to_path_buf());
                }
            }
        }
    }
    return result;
}


pub fn backup_shortcuts(steam_settings:&SteamSettings){
    let backup_folder = get_backups_flder();
    let paths = get_shortcuts_paths(&steam_settings);
    let date = Local::now();
    let date_string = date.format("%Y-%m-%d-%H-%M-%S");
    if let Ok(user_infos) = paths {
        for user_info in user_infos {
            if let Some(shortcut_path) = user_info.shortcut_path {                
                let new_path = backup_folder.join(format!(
                    "{}-{}-shortcuts.vdf",
                    user_info.user_id, 
                    date_string
                ));
                println!("Backed up shortcut at: {:?}", new_path);
                std::fs::copy(&shortcut_path, &new_path).unwrap();
            }
        }
    }
}


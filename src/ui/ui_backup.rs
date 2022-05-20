use crate::{config::get_backups_flder, steam::get_shortcuts_paths};

use super::MyEguiApp;

impl MyEguiApp {
    pub fn render_backup(&mut self, ui: &mut egui::Ui) {
        ui.heading("Backups");

        if ui.button("Back up shortcuts").clicked() {
            let backup_folder = get_backups_flder();
            let paths = get_shortcuts_paths(&self.settings.steam);
            if let Ok(user_infos) = paths {
                for user_info in user_infos {
                    if let Some(shortcut_path) = user_info.shortcut_path {
                        let time_string = "now";
                        let new_path = backup_folder.join(format!(
                            "{}-{}-shortcuts.vdf",
                            user_info.user_id, time_string
                        ));
                        std::fs::copy(&shortcut_path, &new_path).unwrap();
                        println!("Backed up shortcut at: {:?}", new_path);
                    }
                }
            }
        }
    }
}

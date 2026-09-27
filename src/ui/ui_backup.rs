use std::path::PathBuf;

use egui::ScrollArea;

use crate::backups::{backup_shortcuts, load_backups, restore_backup};

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

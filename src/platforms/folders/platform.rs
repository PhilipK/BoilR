use egui::TextBuffer;
use is_executable::is_executable;
use steam_shortcuts_util::Shortcut;

use crate::platforms::{load_settings, FromSettingsString, GamesPlatform, ShortcutToImport};

use super::settings::FoldersSettings;

#[derive(Clone)]
pub struct FoldersPlatform {
    pub settings: FoldersSettings,
}

impl GamesPlatform for FoldersPlatform {
    fn name(&self) -> &str {
        "Folders"
    }

    fn code_name(&self) -> &str {
        "folders"
    }

    fn enabled(&self) -> bool {
        self.settings.enabled
    }

    fn get_shortcut_info(&self) -> eyre::Result<Vec<crate::platforms::ShortcutToImport>> {
        let files = self
            .settings
            .folders
            .iter()
            .flat_map(|f| find_executables_in_path(&f));
        let res = files
            .map(|file| {
                let name = file.path().file_stem().unwrap_or_default().to_string_lossy();
                let path = file.path().to_string_lossy();
                let shortcut = Shortcut::new("0", name.as_str(), path.as_str(), "", "", "", "");
                ShortcutToImport {
                    needs_proton: false,
                    needs_symlinks: false,
                    shortcut: shortcut.to_owned(),
                }
            })
            .collect();
        Ok(res)
    }

    fn get_settings_serilizable(&self) -> String {
        toml::to_string(&self.settings).unwrap_or_default()
    }

    fn render_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Folders");
        ui.checkbox(
            &mut self.settings.enabled,
            "Import from execuables in folders",
        );

        let mut to_remove = vec![];
        for (i,folder) in  self.settings.folders.iter_mut().enumerate() {
            ui.horizontal(|ui|{
                ui.text_edit_singleline(folder);
                if ui.button("Remove folder").clicked(){
                    to_remove.push(i);
                }
            });
        }
        to_remove.reverse();
        for i in to_remove{
            self.settings.folders.remove(i);
        }

        if ui.button("Add folder").clicked(){
            self.settings.folders.push("".to_string());
        }

    }
}

fn find_executables_in_path(path: &str) -> Vec<walkdir::DirEntry> {
    use walkdir::WalkDir;
    WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| is_executable(entry.path()))
        .collect()
}

impl FromSettingsString for FoldersPlatform {
    fn from_settings_string<S: AsRef<str>>(s: S) -> Self {
        FoldersPlatform {
            settings: load_settings(s),
        }
    }
}

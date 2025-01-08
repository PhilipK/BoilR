use super::{get_manifests::get_egs_manifests, EpicPlatform};

impl EpicPlatform {
    pub fn render_epic_settings(&mut self, ui: &mut egui::Ui) {
        let epic_settings = &mut self.settings;
        ui.heading("Epic Games");
        ui.checkbox(&mut epic_settings.enabled, "Import from Epic Games");
        if epic_settings.enabled {
            let safe_mode_header = match epic_settings.safe_launch.len() {
                0 => "Force games to launch through Epic Launcher".to_string(),
                1 => "One game forced to launch through Epic Launcher".to_string(),
                x => format!("{x} games forced to launch through Epic Launcher"),
            };

            egui::CollapsingHeader::new(safe_mode_header)
            .id_salt("Epic_Launcher_safe_launch")
            .show(ui, |ui| {
                ui.label("Some games must be started from the Epic Launcher, select those games below and BoilR will create shortcuts that opens the games through the Epic Launcher.");
                let manifests =self.epic_manifests.get_or_insert_with(||{
                    let manifests = get_egs_manifests(epic_settings);
                    manifests.unwrap_or_default()
                });
                let mut safe_open_games = epic_settings.safe_launch.clone();
                for manifest in manifests{
                    let key = manifest.get_key();
                    let display_name = &manifest.display_name;
                    let mut safe_open = safe_open_games.contains(display_name) || safe_open_games.contains(&key);
                    if ui.checkbox(&mut safe_open, display_name).clicked(){
                        if safe_open{
                            safe_open_games.push(key);
                        }else{
                            safe_open_games.retain(|m| m!= display_name && m!= &key);
                        }
                    }
                }
                epic_settings.safe_launch = safe_open_games;
            });
        }
    }
}

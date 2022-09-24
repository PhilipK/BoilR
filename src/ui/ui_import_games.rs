use eframe::egui;
use egui::ScrollArea;
use futures::executor::block_on;

use tokio::sync::watch;

use crate::config::get_renames_file;
use crate::settings::Settings;
use crate::sync;

use crate::sync::{download_images, SyncProgress};

use super::{backup_shortcuts, ImageSelectState};
use super::{
    ui_colors::{BACKGROUND_COLOR, EXTRA_BACKGROUND_COLOR},
    MyEguiApp,
};

const SECTION_SPACING: f32 = 25.0;

pub enum FetcStatus<T> {
    NeedsFetched,
    Fetching,
    Fetched(T),
}

impl<T> FetcStatus<T> {
    pub fn is_some(&self) -> bool {
        match self {
            FetcStatus::NeedsFetched => false,
            FetcStatus::Fetching => false,
            FetcStatus::Fetched(_) => true,
        }
    }

    pub fn needs_fetching(&self) -> bool {
        match self {
            FetcStatus::NeedsFetched => true,
            FetcStatus::Fetching => false,
            FetcStatus::Fetched(_) => false,
        }
    }
}

impl MyEguiApp {
    pub(crate) fn render_import_games(&mut self, ui: &mut egui::Ui) {
        ui.heading("Import Games");

        self.ensure_games_loaded();

        let mut scroll_style = ui.style_mut();
        scroll_style.visuals.extreme_bg_color = BACKGROUND_COLOR;
        scroll_style.visuals.widgets.inactive.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.widgets.active.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.selection.bg_fill = EXTRA_BACKGROUND_COLOR;
        scroll_style.visuals.widgets.hovered.bg_fill = EXTRA_BACKGROUND_COLOR;

        ScrollArea::vertical()
        .stick_to_right(true)
        .auto_shrink([false,true])
        .show(ui,|ui| {
            ui.reset_style();

            let borrowed_games = &*self.games_to_sync.borrow();
            match borrowed_games{
                FetcStatus::Fetched(games_to_sync) => {
                    ui.label("Select the games you want to import into steam");
                    for (platform_name, shortcuts) in games_to_sync{
                        ui.heading(platform_name);
                        
                        for shortcut in shortcuts {
                            let mut import_game = !self.settings.blacklisted_games.contains(&shortcut.app_id);
                            ui.horizontal(|ui|{
                                if self.current_edit == Option::Some(shortcut.app_id){
                                    if let Some(new_name) = self.rename_map.get_mut(&shortcut.app_id){
                                        ui.text_edit_singleline(new_name).request_focus();
                                        if ui.button("Rename").clicked() {
                                            if new_name.is_empty(){
                                                *new_name = shortcut.app_name.to_string();
                                            }
                                            self.current_edit = Option::None;
                                            let rename_file_path = get_renames_file();
                                            let contents = serde_json::to_string(&self.rename_map);
                                            if let Ok(contents) = contents{
                                                let res = std::fs::write(&rename_file_path, contents);
                                                println!("Write rename file at {:?} with result: {:?}",rename_file_path, res);
                                            }
                                        }
                                    }
                                }  else {                                                          
                                let name = self.rename_map.get(&shortcut.app_id).unwrap_or(&shortcut.app_name);
                                let checkbox = egui::Checkbox::new(&mut import_game,name);
                                let response = ui.add(checkbox);                                
                                if response.double_clicked(){
                                    self.rename_map.entry(shortcut.app_id).or_insert_with(|| shortcut.app_name.to_owned());                                    
                                    self.current_edit = Option::Some(shortcut.app_id);
                                }
                                if response.clicked(){
                                    if !self.settings.blacklisted_games.contains(&shortcut.app_id){
                                        self.settings.blacklisted_games.push(shortcut.app_id);
                                    }else{
                                        self.settings.blacklisted_games.retain(|id| *id != shortcut.app_id);
                                    }
                                }
                            }   
                                
                            });                                                 
                        }
                    }
                    ui.add_space(SECTION_SPACING);
                    ui.label("Check the settings if BoilR didn't find the game you where looking for");
                },
                _=> {
                    ui.ctx().request_repaint();
                    ui.horizontal(|ui|{
                        ui.spinner();                            
                        ui.label("Finding installed games");
                    });
                },
            };
        });
    }

    pub fn ensure_games_loaded(&mut self) {
        if self.games_to_sync.borrow().needs_fetching() {
            self.image_selected_state = ImageSelectState::default();
            let (tx, rx) = watch::channel(FetcStatus::NeedsFetched);
            self.games_to_sync = rx;
            let platforms = self.platforms.clone();
            self.rt.spawn_blocking(move || {
                let _ = tx.send(FetcStatus::Fetching);
                let mut old_shortcuts = vec![];
                for (name,shortcut_info) in sync::get_enum_platform_shortcuts(&platforms){                    
                    old_shortcuts.push((name,shortcut_info));
                }
                let games_to_sync = old_shortcuts;
                let _ = tx.send(FetcStatus::Fetched(games_to_sync));
            });
        }
    }

    pub fn run_sync(&mut self, wait: bool ) {
        let (sender, reciever) = watch::channel(SyncProgress::NotStarted);
        let settings = self.settings.clone();
        if settings.steam.stop_steam {
            crate::steam::ensure_steam_stopped();
        }

        self.status_reciever = reciever;
        let renames = self.rename_map.clone();
        let platforms = self.platforms.clone();
        let handle = self.rt.spawn_blocking(move || {
            MyEguiApp::save_settings_to_file(&settings);
            let mut some_sender = Some(sender);
            backup_shortcuts(&settings.steam);
            let usersinfo = sync::run_sync(&settings, &mut some_sender,&renames,&platforms).unwrap();
            let task = download_images(&settings, &usersinfo, &mut some_sender);
            block_on(task);
            //Run a second time to fix up shortcuts after images are downloaded
            sync::run_sync(&settings, &mut some_sender,&renames,&platforms).unwrap();

            if let Some(sender) = some_sender {
                let _ = sender.send(SyncProgress::Done);
            }
            if settings.steam.start_steam {
                crate::steam::ensure_steam_started(&settings.steam);
            }
        });
        if wait {
            self.rt.block_on(handle).unwrap();
        }
    }

    pub fn save_settings_to_file(settings: &Settings) {
        let toml = toml::to_string(&settings).unwrap();
        let config_path = crate::config::get_config_file();
        std::fs::write(config_path, toml).unwrap();
    }
}

use eframe::egui;
use egui::ScrollArea;
use futures::executor::block_on;

use tokio::sync::watch;

use crate::settings::Settings;
use crate::sync;

use crate::sync::{download_images, SyncProgress};

use super::ImageSelectState;
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
        .stick_to_right()
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
                            let checkbox = egui::Checkbox::new(&mut import_game,&shortcut.app_name);
                            let response = ui.add(checkbox);
                            if response.clicked(){
                                if !self.settings.blacklisted_games.contains(&shortcut.app_id){
                                    self.settings.blacklisted_games.push(shortcut.app_id);
                                }else{
                                    self.settings.blacklisted_games.retain(|id| *id != shortcut.app_id);
                                }
                            }
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
            let settings = self.settings.clone();
            self.rt.spawn_blocking(move || {
                let _ = tx.send(FetcStatus::Fetching);
                let games_to_sync = sync::get_platform_shortcuts(&settings);
                let _ = tx.send(FetcStatus::Fetched(games_to_sync));
            });
        }
    }

    pub fn run_sync(&mut self) {
        let (sender, reciever) = watch::channel(SyncProgress::NotStarted);
        let settings = self.settings.clone();
        if settings.steam.stop_steam {
            crate::steam::ensure_steam_stopped();
        }

        self.status_reciever = reciever;
        self.rt.spawn_blocking(move || {
            MyEguiApp::save_settings_to_file(&settings);
            let mut some_sender = Some(sender);
            let usersinfo = sync::run_sync(&settings, &mut some_sender).unwrap();
            let task = download_images(&settings, &usersinfo, &mut some_sender);
            block_on(task);

            //Run a second time to fix up shortcuts after images are downloaded
            sync::run_sync(&settings, &mut some_sender).unwrap();

            if let Some(sender) = some_sender {
                let _ = sender.send(SyncProgress::Done);
            }
            if settings.steam.start_steam {
                crate::steam::ensure_steam_started(&settings.steam);
            }
        });
    }

    pub fn save_settings_to_file(settings: &Settings) {
        let toml = toml::to_string(&settings).unwrap();
        let config_path = crate::config::get_config_file();
        std::fs::write(config_path, toml).unwrap();
    }
}

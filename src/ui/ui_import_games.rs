use eframe::egui;
use egui::ScrollArea;
use steam_shortcuts_util::shortcut::ShortcutOwned;
use tokio::sync::watch;

use crate::sync;

use super::{
    ui_colors::{BACKGROUND_COLOR, EXTRA_BACKGROUND_COLOR},
    MyEguiApp,
};

const SECTION_SPACING: f32 = 25.0;

pub enum FetchGameStatus {
    NeedsFetched,
    Fetching,
    Fetched(Vec<(String, Vec<ShortcutOwned>)>),
}

impl FetchGameStatus {
    pub fn is_some(&self) -> bool {
        match self {
            FetchGameStatus::NeedsFetched => false,
            FetchGameStatus::Fetching => false,
            FetchGameStatus::Fetched(_) => true,
        }
    }

    pub fn needs_fetching(&self) -> bool {
        match self {
            FetchGameStatus::NeedsFetched => true,
            FetchGameStatus::Fetching => false,
            FetchGameStatus::Fetched(_) => false,
        }
    }
}

impl MyEguiApp {
    pub(crate) fn render_import_games(&mut self, ui: &mut egui::Ui) {
        ui.heading("Import Games");

        if self.games_to_sync.borrow().needs_fetching() {
            let (tx, rx) = watch::channel(FetchGameStatus::NeedsFetched);
            self.games_to_sync = rx;
            let settings = self.settings.clone();
            self.rt.spawn_blocking(move || {
                let _ = tx.send(FetchGameStatus::Fetching);
                let games_to_sync = sync::get_platform_shortcuts(&settings);
                let _ = tx.send(FetchGameStatus::Fetched(games_to_sync));
            });
        }

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
                FetchGameStatus::Fetched(games_to_sync) => {
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
                    ui.label("Finding installed games");
                },
            };
        });
    }
}

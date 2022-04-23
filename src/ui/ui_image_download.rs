use std::path::Path;

use egui::{ImageButton, ScrollArea};
use steam_shortcuts_util::shortcut::ShortcutOwned;

use crate::{
    steam::{get_shortcuts_paths, SteamUsersInfo},
    steamgriddb::{ImageType, CachedSearch},
};

use super::{ui_images::load_image_from_path, MyEguiApp};

#[derive(Default, PartialEq)]
pub struct ImageSelectState {
    pub selected_image: Option<ShortcutOwned>,
    pub grid_id: Option<usize>,

    pub hero_image: Option<egui::TextureHandle>,
    pub grid_image: Option<egui::TextureHandle>,
    pub logo_image: Option<egui::TextureHandle>,
    pub icon_image: Option<egui::TextureHandle>,
    pub wide_image: Option<egui::TextureHandle>,

    pub steam_user: Option<SteamUsersInfo>,
    pub steam_users: Option<Vec<SteamUsersInfo>>,
    
    

    
}

impl MyEguiApp {
    pub(crate) fn render_ui_images(&mut self, ui: &mut egui::Ui) {
        self.ensure_games_loaded();

        ui.heading("Images");

        match &self.image_selected_state.steam_user {
            Some(user) => {
                ScrollArea::vertical()
                    .stick_to_right()
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.reset_style();
                        let borrowed_games = &*self.games_to_sync.borrow();
                        match borrowed_games {
                            super::FetcStatus::Fetched(games_to_sync) => {
                                match &self.image_selected_state.selected_image {
                                    Some(selected_image) => {
                                        if ui.button("Back").clicked() {
                                            self.image_selected_state.selected_image = None;
                                            return;
                                        }
                                        ui.heading(&selected_image.app_name);

                                        if let Some(grid_id) = self.image_selected_state.grid_id{
                                            ui.horizontal(|ui|{
                                                ui.label("Grid id:");
                                                let mut text_id = format!("{}",grid_id);
                                                if ui.text_edit_singleline(&mut text_id).changed(){
                                                    if let Ok(grid_id) = text_id.parse::<usize>(){
                                                        if let Some(auth_key ) = &self.settings.steamgrid_db.auth_key{
                                                            let client = steamgriddb_api::Client::new(auth_key);
                                                            let mut search = CachedSearch::new(&client);
                                                            search.set_cache(selected_image.app_id, selected_image.app_name.to_string(), grid_id);
                                                        }
                                                        self.image_selected_state.grid_id = Some(grid_id);
                                                    }
                                                };
                                            });
                                        }

                                        ui.label("Hero");
                                        render_image(ui,&mut self.image_selected_state.hero_image);

                                        ui.label("Grid");
                                        render_image(ui,&mut self.image_selected_state.grid_image);

                                        ui.label("Icon");
                                        render_image(ui,&mut self.image_selected_state.icon_image);

                                        ui.label("Logo");
                                        render_image(ui,&mut self.image_selected_state.logo_image);                                      

                                        ui.label("Wide");
                                        render_image(ui,&mut self.image_selected_state.wide_image);
                                    }
                                    None => {
                                        for (platform_name, shortcuts) in games_to_sync {
                                            ui.heading(platform_name);
                                            for shortcut in shortcuts {
                                                if ui.button(&shortcut.app_name).clicked() {

                                                    if let Some(auth_key ) = &self.settings.steamgrid_db.auth_key{
                                                        let client = steamgriddb_api::Client::new(auth_key);
                                                        let search = CachedSearch::new(&client);
                                                        //TODO make this multithreaded
                                                        self.image_selected_state.grid_id = self.rt.block_on(search.search(shortcut.app_id, &shortcut.app_name)).ok().flatten();
                                                    }

                                                    self.image_selected_state.selected_image =
                                                        Some(shortcut.clone());

                                                    let folder =
                                                        Path::new(&user.steam_user_data_folder)
                                                            .join("config")
                                                            .join("grid");

                                                    //TODO put this in seperate thread
                                                    self.image_selected_state.hero_image =
                                                        get_image(
                                                            ui,
                                                            shortcut,
                                                            &folder,
                                                            &ImageType::Hero,
                                                        );
                                                    self.image_selected_state.grid_image =
                                                        get_image(
                                                            ui,
                                                            shortcut,
                                                            &folder,
                                                            &ImageType::Grid,
                                                        );
                                                    self.image_selected_state.icon_image =
                                                        get_image(
                                                            ui,
                                                            shortcut,
                                                            &folder,
                                                            &ImageType::Icon,
                                                        );
                                                    self.image_selected_state.logo_image =
                                                        get_image(
                                                            ui,
                                                            shortcut,
                                                            &folder,
                                                            &ImageType::Logo,
                                                        );
                                                    self.image_selected_state.wide_image =
                                                        get_image(
                                                            ui,
                                                            shortcut,
                                                            &folder,
                                                            &ImageType::WideGrid,
                                                        );
                                                };
                                            }
                                        }
                                    }
                                }
                            }
                            _ => {
                                ui.label("Finding installed games");
                            }
                        }
                    });
            }
            None => {
                let users = self
                    .image_selected_state
                    .steam_users
                    .get_or_insert_with(|| {
                        get_shortcuts_paths(&self.settings.steam).expect("Should have steam user")
                    });
                for user in users {
                    if ui.button(&user.user_id).clicked() {
                        self.image_selected_state.steam_user = Some(user.clone());
                    }
                }
            }
        }
    }


}

fn render_image(ui: &mut egui::Ui, image: &mut Option<egui::TextureHandle>) -> bool {
    match image {
        Some(texture) => {
            let size = texture.size_vec2();
            let image_button =
                ImageButton::new(texture, size * 0.1);
            ui.add(image_button)
                .on_hover_text("Click to change image").clicked()
        }
        None => {
            ui.button("Pick an image").clicked()
        }
    }
}

fn get_image(
    ui: &mut egui::Ui,
    shortcut: &ShortcutOwned,
    folder: &std::path::Path,
    image_type: &ImageType,
) -> Option<egui::TextureHandle> {
    let file_name = ImageType::file_name(image_type, shortcut.app_id);
    let file_path = folder.join(file_name);
    let image = load_image_from_path(file_path.as_path()).map(|img_data| {
        ui.ctx()
            .load_texture(file_path.to_string_lossy().to_string(), img_data)
    });
    image
}

use std::path::Path;

use egui::{ScrollArea, ImageButton};
use steam_shortcuts_util::shortcut::ShortcutOwned;

use crate::{steamgriddb::ImageType, steam::{SteamUsersInfo, get_shortcuts_paths}};

use super::{MyEguiApp, ui_images::load_image_from_path};

#[derive(Default, PartialEq)]
pub struct ImageSelectState{
    pub selected_image : Option<ShortcutOwned>,
    pub hero_image: Option<egui::TextureHandle>,
    pub steam_user: Option<SteamUsersInfo>,
    pub steam_users: Option<Vec<SteamUsersInfo>>
}

impl MyEguiApp {
    pub(crate) fn render_ui_images(&mut self, ui: &mut egui::Ui) {
        
        self.ensure_games_loaded();    
        
        ui.heading("Images");


        match &self.image_selected_state.steam_user {
            Some(user) => {
                ScrollArea::vertical()
                .stick_to_right()
                .auto_shrink([false,true])
                .show(ui,|ui| {
                    ui.reset_style();
                    let borrowed_games = &*self.games_to_sync.borrow();
                    match borrowed_games{
                        super::FetchGameStatus::Fetched(games_to_sync) => {
                            let mut un_select = false;
                            match &self.image_selected_state.selected_image{
                                Some(selected_image) => {
                                    if ui.button("Back").clicked(){
                                        un_select=true;
                                    }
                                    ui.heading(&selected_image.app_name);
                                    ui.label("Hero Image");
                                    match &mut self.image_selected_state.hero_image{
                                        Some(texture) => {
                                            let size = texture.size_vec2();
                                            let image_button = ImageButton::new(texture, size * 0.1);
                                             ui
                                                .add(image_button)
                                                .on_hover_text("Import your games into steam");
                                           
                                        },
                                        None => {ui.label("No image");},
                                    }
                                },
                                None => {
                                    for (platform_name, shortcuts) in games_to_sync{
                                        ui.heading(platform_name);
                                        for shortcut in shortcuts{
                                            if ui.button(&shortcut.app_name).clicked(){
                                                self.image_selected_state.selected_image = Some(shortcut.clone());
        
                                                let folder = Path::new(&user.steam_user_data_folder).join("config").join("grid");
                                                //TODO put this in seperate thread
                                                let file_name = ImageType::file_name(&ImageType::Hero, shortcut.app_id);
                                                //TODO get this for real
                                                let hero_file = folder.join(file_name);
                                                dbg!(&hero_file);
                                                if let Some(img_data) =load_image_from_path(hero_file.as_path()){
                                                    self.image_selected_state.hero_image= Some(ui.ctx().load_texture(format!("hero_{}",shortcut.app_id), img_data));
                                                }
                                            };
                                        }
                                    }
                                },
                            }
                            if un_select{
                                self.image_selected_state.selected_image= None;
                            }
                            
                        },
                        _ => {
                            ui.label("Finding installed games");
        
                        },
                    }
        
                });
        
            },
            None => {
                let users = self.image_selected_state.steam_users.get_or_insert_with(||{
                    get_shortcuts_paths(&self.settings.steam).expect("Should have steam user")
                });
                for user in users{
                    if ui.button(&user.user_id).clicked(){
                        self.image_selected_state.steam_user = Some(user.clone());
                    }
                }
            }
        }
     



      
    }
}
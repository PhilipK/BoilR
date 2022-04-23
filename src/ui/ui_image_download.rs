use std::path::{Path, PathBuf};

use crate::{
    steam::{get_shortcuts_paths, SteamUsersInfo},
    steamgriddb::{CachedSearch, ImageType, get_query_type, ToDownload},
};
use dashmap::DashMap;
use egui::{ImageButton, ScrollArea};
use futures::executor::block_on;
use steam_shortcuts_util::shortcut::ShortcutOwned;
use tokio::sync::watch::{self, Receiver};

use super::{ui_images::load_image_from_path, FetcStatus, MyEguiApp};

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

    pub image_to_replace: Option<ImageType>,
    pub image_options: Receiver<FetcStatus<Vec<PathBuf>>>,

    pub image_handles: DashMap<String,egui::TextureHandle>,
}

impl Default for ImageSelectState {
    fn default() -> Self {
        Self {
            selected_image: Default::default(),
            grid_id: Default::default(),
            hero_image: Default::default(),
            grid_image: Default::default(),
            logo_image: Default::default(),
            icon_image: Default::default(),
            wide_image: Default::default(),
            steam_user: Default::default(),
            steam_users: Default::default(),
            image_to_replace: Default::default(),
            image_options: watch::channel(FetcStatus::NeedsFetched).1,
            image_handles: DashMap::new()
        }
    }
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
                                        let mut reset = false;
                                        if let Some(selected_image_type) =
                                            &self.image_selected_state.image_to_replace
                                        {
                                            let borrowed_images =
                                                &*self.image_selected_state.image_options.borrow();
                                            match borrowed_images {
                                                FetcStatus::Fetched(images) => {
                                                    for image in images{
                                                        let image_key = image.as_path().to_string_lossy().to_string();
                                                        if ! self.image_selected_state.image_handles.contains_key(&image_key){
                                                            //TODO remove this unwrap
                                                            let image_data = load_image_from_path(&image).unwrap();
                                                            let handle = ui.ctx().load_texture(&image_key, image_data);
                                                            self.image_selected_state.image_handles.insert(image_key.clone(),handle);
                                                        }
                                                        if let Some(texture_handle) = self.image_selected_state.image_handles.get(&image_key){
                                                            let size = texture_handle.size_vec2();
                                                            let image_button = ImageButton::new(texture_handle.value(), size);
                                                            if ui.add(image_button).clicked(){
                                                                 let to =
                                                                    Path::new(&user.steam_user_data_folder)
                                                                .join("config")
                                                                .join("grid")
                                                                .join(selected_image_type.file_name(selected_image.app_id));                                                            
                                                                let _ = std::fs::copy(image, to);
                                                                reset = true;
                                                            }
                                                        }
                                                    }
                                                },
                                                _ => {
                                                    ui.label("Finding possible images");
                                                },
                                                
                                            }
                                        } else {
                                            if let Some(grid_id) = self.image_selected_state.grid_id
                                            {
                                                ui.horizontal(|ui| {
                                                    ui.label("Grid id:");
                                                    let mut text_id = format!("{}", grid_id);
                                                    if ui
                                                        .text_edit_singleline(&mut text_id)
                                                        .changed()
                                                    {
                                                        if let Ok(grid_id) =
                                                            text_id.parse::<usize>()
                                                        {
                                                            if let Some(auth_key) =
                                                                &self.settings.steamgrid_db.auth_key
                                                            {
                                                                let client =
                                                                    steamgriddb_api::Client::new(
                                                                        auth_key,
                                                                    );
                                                                let mut search =
                                                                    CachedSearch::new(&client);
                                                                search.set_cache(
                                                                    selected_image.app_id,
                                                                    selected_image
                                                                        .app_name
                                                                        .to_string(),
                                                                    grid_id,
                                                                );
                                                            }
                                                            self.image_selected_state.grid_id =
                                                                Some(grid_id);
                                                        }
                                                    };
                                                });
                                            }

                                            for image_type in ImageType::all() {
                                                ui.label(image_type.name());

                                                let image_ref = match image_type {
                                                    ImageType::Hero => {
                                                        &mut self.image_selected_state.hero_image
                                                    }
                                                    ImageType::Grid => {
                                                        &mut self.image_selected_state.grid_image
                                                    }
                                                    ImageType::WideGrid => {
                                                        &mut self.image_selected_state.wide_image
                                                    }
                                                    ImageType::Logo => {
                                                        &mut self.image_selected_state.logo_image
                                                    }
                                                    ImageType::BigPicture => {
                                                        &mut self.image_selected_state.wide_image
                                                    }
                                                    ImageType::Icon => {
                                                        &mut self.image_selected_state.icon_image
                                                    }
                                                };
                                                if render_image(ui, image_ref) {
                                                    self.image_selected_state.image_to_replace =
                                                        Some(image_type.clone());
                                                    let (mut tx,rx)=  watch::channel(FetcStatus::Fetching);
                                                    self.image_selected_state.image_options = rx;
                                                    let settings = self.settings.clone();
                                                    if let Some(auth_key) = settings.steamgrid_db.auth_key{
                                                        if let Some(grid_id) = self.image_selected_state.grid_id{
                                                            let auth_key = auth_key.clone();
                                                            let image_type = image_type.clone();
                                                            let app_name = selected_image.app_name.clone();
                                                            self.rt.spawn_blocking( move || {
                                                                //Find somewhere else to put this
                                                                std::fs::create_dir_all(".thumbnails");
                                                                let thumbnails_folder = Path::new(".thumbnails");
                                                                let client =steamgriddb_api::Client::new(auth_key);
                                                                let query = get_query_type(false,&image_type);
                                                                let search_res = block_on(client.get_images_for_id(grid_id, &query));

                                                                if let Ok(possible_images) = search_res{
                                                                    let mut result = vec![];
                                                                    for possible_image in &possible_images{
                                                                        let path = thumbnails_folder.join(format!("{}.png",possible_image.id));
                                                                        if !&path.exists(){
                                                                            let to_download = ToDownload{
                                                                                path: path.clone(),
                                                                                url: possible_image.thumb.clone(),
                                                                                app_name: app_name.clone(),
                                                                                image_type: image_type.clone()
                                                                            };
                                                                            //TODO make this actually parallel
                                                                            block_on(crate::steamgriddb::download_to_download(&to_download));
                                                                        }
                                                                        result.push(path);                                                                        
                                                                    }
                                                                    let _ = tx.send(FetcStatus::Fetched(result));
                                                                }
                                                            });
                                                        }
                                                    };
                                                }
                                            }
                                        }
                                        if reset {
                                            self.image_selected_state.image_to_replace = None;
                                        }
                                    }
                                    None => {
                                        for (platform_name, shortcuts) in games_to_sync {
                                            ui.heading(platform_name);
                                            for shortcut in shortcuts {
                                                if ui.button(&shortcut.app_name).clicked() {
                                                    if let Some(auth_key) =
                                                        &self.settings.steamgrid_db.auth_key
                                                    {
                                                        let client =
                                                            steamgriddb_api::Client::new(auth_key);
                                                        let search = CachedSearch::new(&client);
                                                        //TODO make this multithreaded
                                                        self.image_selected_state.grid_id = self
                                                            .rt
                                                            .block_on(search.search(
                                                                shortcut.app_id,
                                                                &shortcut.app_name,
                                                            ))
                                                            .ok()
                                                            .flatten();
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
            let image_button = ImageButton::new(texture, size * 0.1);
            ui.add(image_button)
                .on_hover_text("Click to change image")
                .clicked()
        }
        None => ui.button("Pick an image").clicked(),
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

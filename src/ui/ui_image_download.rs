use std::{
    ops::ControlFlow,
    path::{Path, PathBuf},
};

use crate::{
    steam::{get_shortcuts_paths, SteamUsersInfo},
    steamgriddb::{get_query_type, CachedSearch, ImageType, ToDownload},
};
use dashmap::DashMap;
use egui::{ImageButton, ScrollArea, TextureHandle};
use futures::executor::block_on;
use steam_shortcuts_util::shortcut::ShortcutOwned;
use tokio::sync::watch::{self, Receiver};

use super::{ui_images::load_image_from_path, FetcStatus, MyEguiApp};

pub struct ImageSelectState {
    pub selected_shortcut: Option<ShortcutOwned>,
    pub grid_id: Option<usize>,

    pub hero_image: Option<egui::TextureHandle>,
    pub grid_image: Option<egui::TextureHandle>,
    pub logo_image: Option<egui::TextureHandle>,
    pub icon_image: Option<egui::TextureHandle>,
    pub wide_image: Option<egui::TextureHandle>,

    pub steam_user: Option<SteamUsersInfo>,
    pub steam_users: Option<Vec<SteamUsersInfo>>,

    pub image_type_selected: Option<ImageType>,
    pub image_options: Receiver<FetcStatus<Vec<PossibleImage>>>,

    pub image_handles: DashMap<String, egui::TextureHandle>,
}

impl ImageSelectState {
    pub fn has_multiple_users(&self) -> bool {
        match &self.steam_users {
            Some(users) => users.len() > 1,
            None => false,
        }
    }
}

#[derive(Clone)]
pub struct PossibleImage {
    thumbnail_path: PathBuf,
    full_url: String,
}

impl Default for ImageSelectState {
    fn default() -> Self {
        Self {
            selected_shortcut: Default::default(),
            grid_id: Default::default(),
            hero_image: Default::default(),
            grid_image: Default::default(),
            logo_image: Default::default(),
            icon_image: Default::default(),
            wide_image: Default::default(),
            steam_user: Default::default(),
            steam_users: Default::default(),
            image_type_selected: Default::default(),
            image_options: watch::channel(FetcStatus::NeedsFetched).1,
            image_handles: DashMap::new(),
        }
    }
}

enum UserAction {
    UserSelected(SteamUsersInfo),
    ShortcutSelected(ShortcutOwned),
    ImageTypeSelected(ImageType),
    ImageSelected(PossibleImage),
    BackButton,
    NoAction,
}

impl MyEguiApp {
    fn render_ui_image_action(&self, ui: &mut egui::Ui) -> UserAction {
        let state = &self.image_selected_state;
        ui.heading("Images");
        if state.selected_shortcut.is_some() || state.has_multiple_users() {
            if ui.button("Back").clicked() {
                return UserAction::BackButton;
            }
        }
        if state.steam_user.is_none() {
            //We always ensure that users are there before this call
            let users = state.steam_users.as_ref().unwrap();
            if users.len() == 1 {
                return UserAction::UserSelected(users[0].clone());
            }
            for user in users {
                if ui.button(&user.user_id).clicked() {
                    return UserAction::UserSelected(user.clone());
                }
            }
            return UserAction::NoAction;
        }
        if let Some(shortcut) = state.selected_shortcut.as_ref() {
            ui.heading(&shortcut.app_name);
            if let Some(image_type) = state.image_type_selected.as_ref(){
                ui.heading(image_type.name());
                match &*state.image_options.borrow(){
                    FetcStatus::Fetched(images) => {
                        for image in images {
                            let image_key = image
                                .thumbnail_path
                                .as_path()
                                .to_string_lossy()
                                .to_string();
                        //TODO continue implementing this

                        }
                    },
                    _ => {ui.label("Finding possible images");},
                }

            }else{
                for image_type in ImageType::all() {
                    ui.label(image_type.name());
                    let image_ref = get_image_ref(image_type,state);
                    if render_thumbnail(ui, image_ref){
                        return UserAction::ImageTypeSelected(image_type.clone());
                    }
                }
            }
        } else {
            match &*self.games_to_sync.borrow() {
                FetcStatus::Fetched(shortcuts) => {
                    for (platform_name, shortcuts) in shortcuts {
                        ui.heading(platform_name);
                        for shortcut in shortcuts {
                            if ui.button(&shortcut.app_name).clicked() {
                                return UserAction::ShortcutSelected(shortcut.clone());
                            }
                        }
                    }
                }
                _ => {
                    ui.label("Finding installed games");
                }
            }
        }
        UserAction::NoAction
    }

    

    fn ensure_steam_users_loaded(&mut self) {
        self.image_selected_state
            .steam_users
            .get_or_insert_with(|| {
                get_shortcuts_paths(&self.settings.steam).expect("Should have steam user")
            });
    }

    pub(crate) fn render_ui_images(&mut self, ui: &mut egui::Ui) {
        self.ensure_games_loaded();
        self.ensure_steam_users_loaded();

        let mut action = UserAction::NoAction;
        ScrollArea::vertical()
            .stick_to_right()
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.reset_style();
                action = self.render_ui_image_action(ui);
            });
        match action {
            UserAction::UserSelected(user) => {
                let state = &mut self.image_selected_state;
                state.steam_user = Some(user);
            }
            UserAction::ShortcutSelected(shortcut) => {
                self.handle_shortcut_selected(shortcut, ui);
            }
            UserAction::ImageTypeSelected(image_type) => {
                let state = &mut self.image_selected_state;
                //TODO also load possible images
                state.image_type_selected = Some(image_type);
            }
            UserAction::ImageSelected(_) => todo!(),
            UserAction::BackButton => {
                let state = &mut self.image_selected_state;
                handle_back_button_action(state);
            }
            UserAction::NoAction => {}
        };

        match &self.image_selected_state.steam_user {
            Some(user) => {
                let borrowed_games = &*self.games_to_sync.borrow();
                match borrowed_games {
                    super::FetcStatus::Fetched(games_to_sync) => {
                        match &self.image_selected_state.selected_shortcut {
                            Some(selected_image) => {
                                ui.heading(&selected_image.app_name);
                                let mut reset = false;
                                if let Some(selected_image_type) =
                                    &self.image_selected_state.image_type_selected
                                {
                                    let borrowed_images =
                                        &*self.image_selected_state.image_options.borrow();
                                    match borrowed_images {
                                        FetcStatus::Fetched(images) => {
                                            for image in images {
                                                let image_key = image
                                                    .thumbnail_path
                                                    .as_path()
                                                    .to_string_lossy()
                                                    .to_string();
                                                if !self
                                                    .image_selected_state
                                                    .image_handles
                                                    .contains_key(&image_key)
                                                {
                                                    //TODO remove this unwrap
                                                    let image_data =
                                                        load_image_from_path(&image.thumbnail_path)
                                                            .unwrap();
                                                    let handle = ui
                                                        .ctx()
                                                        .load_texture(&image_key, image_data);
                                                    self.image_selected_state
                                                        .image_handles
                                                        .insert(image_key.clone(), handle);
                                                }
                                                if let Some(texture_handle) = self
                                                    .image_selected_state
                                                    .image_handles
                                                    .get(&image_key)
                                                {
                                                    let mut size = texture_handle.size_vec2();
                                                    clamp_to_width(&mut size, MAX_WIDTH);
                                                    let image_button = ImageButton::new(
                                                        texture_handle.value(),
                                                        size,
                                                    );
                                                    if ui.add(image_button).clicked() {
                                                        let to =
                                                            Path::new(&user.steam_user_data_folder)
                                                                .join("config")
                                                                .join("grid")
                                                                .join(
                                                                    selected_image_type.file_name(
                                                                        selected_image.app_id,
                                                                    ),
                                                                );
                                                        let app_name =
                                                            selected_image.app_name.clone();

                                                        let to_download = ToDownload {
                                                            path: to,
                                                            url: image.full_url.clone(),
                                                            app_name: app_name.clone(),
                                                            image_type: selected_image_type.clone(),
                                                        };
                                                        //TODO make this actually parallel
                                                        self.rt.spawn_blocking(move ||{
                                                                    let _ = block_on(crate::steamgriddb::download_to_download(&to_download));
                                                                });

                                                        let image_ref = match selected_image_type {
                                                            ImageType::Hero => {
                                                                &mut self
                                                                    .image_selected_state
                                                                    .hero_image
                                                            }
                                                            ImageType::Grid => {
                                                                &mut self
                                                                    .image_selected_state
                                                                    .grid_image
                                                            }
                                                            ImageType::WideGrid => {
                                                                &mut self
                                                                    .image_selected_state
                                                                    .wide_image
                                                            }
                                                            ImageType::Logo => {
                                                                &mut self
                                                                    .image_selected_state
                                                                    .logo_image
                                                            }
                                                            ImageType::BigPicture => {
                                                                &mut self
                                                                    .image_selected_state
                                                                    .wide_image
                                                            }
                                                            ImageType::Icon => {
                                                                &mut self
                                                                    .image_selected_state
                                                                    .icon_image
                                                            }
                                                        };
                                                        *image_ref = Some(texture_handle.clone());
                                                        reset = true;
                                                    }
                                                }
                                            }
                                        }
                                        _ => {
                                            ui.label("Finding possible images");
                                        }
                                    }
                                } else {
                                    if let Some(grid_id) = self.image_selected_state.grid_id {
                                        ui.horizontal(|ui| {
                                            ui.label("Grid id:");
                                            let mut text_id = format!("{}", grid_id);
                                            if ui.text_edit_singleline(&mut text_id).changed() {
                                                if let Ok(grid_id) = text_id.parse::<usize>() {
                                                    if let Some(auth_key) =
                                                        &self.settings.steamgrid_db.auth_key
                                                    {
                                                        let client =
                                                            steamgriddb_api::Client::new(auth_key);
                                                        let mut search = CachedSearch::new(&client);
                                                        search.set_cache(
                                                            selected_image.app_id,
                                                            selected_image.app_name.to_string(),
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
                                        if render_thumbnail(ui, image_ref) {
                                            self.image_selected_state.image_type_selected =
                                                Some(image_type.clone());
                                            let (mut tx, rx) = watch::channel(FetcStatus::Fetching);
                                            self.image_selected_state.image_options = rx;
                                            let settings = self.settings.clone();
                                            if let Some(auth_key) = settings.steamgrid_db.auth_key {
                                                if let Some(grid_id) =
                                                    self.image_selected_state.grid_id
                                                {
                                                    let auth_key = auth_key.clone();
                                                    let image_type = image_type.clone();
                                                    let app_name = selected_image.app_name.clone();
                                                    self.rt.spawn_blocking( move|| {
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
                                                                        result.push(PossibleImage { thumbnail_path: path, full_url: possible_image.url.clone() });
                                                                        let _ = tx.send(FetcStatus::Fetched(result.clone()));
                                                                    }
                                                                }
                                                            });
                                                }
                                            };
                                        }
                                    }
                                }
                                if reset {
                                    self.image_selected_state.image_type_selected = None;
                                    self.image_selected_state.image_options =
                                        watch::channel(FetcStatus::NeedsFetched).1;
                                    self.image_selected_state.image_handles.clear();
                                }
                            }
                            None => {
                            }
                        }
                    }
                    _ => {
                        ui.label("Finding installed games");
                    }
                }
            }
            None => {}
        }
    }

    fn handle_shortcut_selected(&mut self,  shortcut: ShortcutOwned, ui: &mut egui::Ui) {
        let state = &mut self.image_selected_state;
        //We must have a user to make see this action;
        let user = state.steam_user.as_ref().unwrap();
        if let Some(auth_key) = &self.settings.steamgrid_db.auth_key {
            let client = steamgriddb_api::Client::new(auth_key);
            let search = CachedSearch::new(&client);
            //TODO make this multithreaded
            state.grid_id = self
                .rt
                .block_on(search.search(shortcut.app_id, &shortcut.app_name))
                .ok()
                .flatten();
        }
        state.selected_shortcut = Some(shortcut.clone());
        let folder = Path::new(&user.steam_user_data_folder)
            .join("config")
            .join("grid");
        //TODO put this in seperate thread
        state.hero_image = get_image(ui, &shortcut, &folder, &ImageType::Hero);
        state.grid_image = get_image(ui, &shortcut, &folder, &ImageType::Grid);
        state.icon_image = get_image(ui, &shortcut, &folder, &ImageType::Icon);
        state.logo_image = get_image(ui, &shortcut, &folder, &ImageType::Logo);
        state.wide_image = get_image(ui, &shortcut, &folder, &ImageType::WideGrid);
        state.selected_shortcut = Some(shortcut);
    }
}

fn handle_back_button_action(state: &mut ImageSelectState) {
    if state.image_type_selected.is_some() {
        state.image_type_selected = None;
    } else {
        if state.selected_shortcut.is_some() {
            state.selected_shortcut = None;
        } else {
            state.steam_user = None;
        }
    }
}

const MAX_WIDTH: f32 = 300.;

fn render_thumbnail(ui: &mut egui::Ui, image: &Option<egui::TextureHandle>) -> bool {
    match image {
        Some(texture) => {
            let mut size = texture.size_vec2();
            clamp_to_width(&mut size, MAX_WIDTH);
            let image_button = ImageButton::new(texture, size);
            ui.add(image_button)
                .on_hover_text("Click to change image")
                .clicked()
        }
        None => ui.button("Pick an image").clicked(),
    }
}

fn clamp_to_width(size: &mut egui::Vec2, max_width: f32) {
    let mut x = size.x;
    let mut y = size.y;
    if size.x > max_width {
        let ratio = size.y / size.x;
        x = max_width;
        y = x * ratio;
    }
    size.x = x;
    size.y = y;
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


fn get_image_ref<'a>(image_type: &ImageType, state: &'a ImageSelectState) -> &'a Option<TextureHandle>{
   match  image_type {
        ImageType::Hero => {
            &state.hero_image
        }
        ImageType::Grid => {
            &state.grid_image
        }
        ImageType::WideGrid => {
            &state.wide_image
        }
        ImageType::Logo => {
            &state.logo_image
        }
        ImageType::BigPicture => {
            &state.wide_image
        }
        ImageType::Icon => {
            &state.icon_image
        }
    }
}
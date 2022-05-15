use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    config::get_thumbnails_folder,
    steam::{get_installed_games, SteamGameInfo},
    steam::{get_shortcuts_paths, SteamUsersInfo},
    steamgriddb::{get_image_extension, get_query_type, CachedSearch, ImageType, ToDownload},
};
use dashmap::DashMap;
use egui::{ImageButton, ScrollArea};
use futures::executor::block_on;
use steam_shortcuts_util::shortcut::ShortcutOwned;
use steamgriddb_api::images::MimeTypes;
use tokio::sync::watch::{self, Receiver};

use super::{ui_images::load_image_from_path, FetcStatus, MyEguiApp};

pub struct ImageSelectState {
    pub selected_shortcut: Option<GameType>,
    pub grid_id: Option<usize>,

    pub steam_user: Option<SteamUsersInfo>,
    pub steam_users: Option<Vec<SteamUsersInfo>>,
    pub user_shortcuts: Option<Vec<ShortcutOwned>>,
    pub game_mode: GameMode,
    pub image_type_selected: Option<ImageType>,
    pub image_options: Receiver<FetcStatus<Vec<PossibleImage>>>,
    pub steam_games: Option<Vec<crate::steam::SteamGameInfo>>,
    pub image_handles: std::sync::Arc<DashMap<String, TextureState>>,

    pub possible_names: Option<Vec<steamgriddb_api::search::SearchResult>>,
}

#[derive(Clone)]
pub enum TextureState {
    Downloading,
    Downloaded,
    Loaded(egui::TextureHandle),
}

#[derive(Debug)]
pub enum GameMode {
    Shortcuts,
    SteamGames,
}

impl GameMode {
    pub fn is_shortcuts(&self) -> bool {
        match self {
            GameMode::Shortcuts => true,
            GameMode::SteamGames => false,
        }
    }
}

impl ImageSelectState {
    pub fn has_multiple_users(&self) -> bool {
        match &self.steam_users {
            Some(users) => users.len() > 1,
            None => false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct PossibleImage {
    thumbnail_path: PathBuf,
    thumbnail_url: String,
    mime: MimeTypes,
    full_url: String,
    id: u32,
}

impl Default for ImageSelectState {
    fn default() -> Self {
        Self {
            selected_shortcut: Default::default(),
            grid_id: Default::default(),
            steam_user: Default::default(),
            steam_users: Default::default(),
            user_shortcuts: Default::default(),
            game_mode: GameMode::Shortcuts,
            image_type_selected: Default::default(),
            possible_names: None,
            image_options: watch::channel(FetcStatus::NeedsFetched).1,
            image_handles: Arc::new(DashMap::new()),
            steam_games: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum GameType {
    Shortcut(ShortcutOwned),
    SteamGame(SteamGameInfo),
}

impl GameType {
    pub fn app_id(&self) -> u32 {
        match self {
            GameType::Shortcut(shortcut) => shortcut.app_id,
            GameType::SteamGame(game) => game.appid,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            GameType::Shortcut(s) => s.app_name.as_ref(),
            GameType::SteamGame(g) => g.name.as_ref(),
        }
    }
}

#[derive(Debug)]
enum UserAction {
    CorrectGridId,
    UserSelected(SteamUsersInfo),
    ShortcutSelected(GameType),
    ImageTypeSelected(ImageType),
    ImageTypeCleared(ImageType, bool),
    ImageSelected(PossibleImage),
    GridIdChanged(usize),
    SetGamesMode(GameMode),
    BackButton,
    NoAction,
}

impl MyEguiApp {
    fn render_ui_image_action(&self, ui: &mut egui::Ui) -> UserAction {
        let state = &self.image_selected_state;
        ui.heading("Images");
        if (state.selected_shortcut.is_some()
            || (state.has_multiple_users() && state.steam_user.is_some()))
            && ui.button("Back").clicked()
        {
            return UserAction::BackButton;
        }
        if state.steam_user.is_none() {
            return render_user_select(state, ui);
        }
        if let Some(shortcut) = state.selected_shortcut.as_ref() {
            ui.heading(shortcut.name());

            if let Some(possible_names) = state.possible_names.as_ref() {
                if let Some(value) = render_possible_names(possible_names, ui) {
                    return value;
                }
            } else {
                if let Some(image_type) = state.image_type_selected.as_ref() {
                    if let Some(action) = self.render_possible_images(ui, image_type, state) {
                        return action;
                    }
                } else {
                    if let Some(action) = render_shortcut_images(ui, state) {
                        return action;
                    }
                }
            }
        } else {
            let is_shortcut = state.game_mode.is_shortcuts();
            if ui
                .selectable_label(is_shortcut, "Images for shortcuts")
                .clicked()
            {
                return UserAction::SetGamesMode(GameMode::Shortcuts);
            }

            if ui
                .selectable_label(!is_shortcut, "Images for steam games")
                .clicked()
            {
                return UserAction::SetGamesMode(GameMode::SteamGames);
            }
            if is_shortcut {
                if let Some(action) = self.render_shortcut_select(ui) {
                    return action;
                }
            } else {
                if let Some(action) = render_steam_game_select(ui, state) {
                    return action;
                }
            }
        }
        UserAction::NoAction
    }

    fn render_shortcut_select(&self, ui: &mut egui::Ui) -> Option<UserAction> {
        let shortcuts = &self.image_selected_state.user_shortcuts;

        match shortcuts {
            Some(shortcuts) => {
                let user_info = &self.image_selected_state.steam_user.as_ref().unwrap();
                for shortcut in shortcuts {
                    let (_, key) = shortcut.key(
                        &ImageType::Grid,
                        Path::new(&user_info.steam_user_data_folder),
                    );
                    let texture = self.image_selected_state.image_handles.get(&key);
                    let mut clicked = false;
                    if let Some(texture) = texture {
                        match &texture.value() {
                            TextureState::Loaded(texture) => {
                                let mut size = texture.size_vec2();
                                clamp_to_width(&mut size, 100.);
                                let image_button = ImageButton::new(texture, size);
                                clicked = clicked || ui.add(image_button).clicked();
                            }
                            _ => {}
                        }
                    }

                    let button = ui.button(&shortcut.app_name);
                    clicked = clicked || button.clicked();
                    if clicked {
                        return Some(UserAction::ShortcutSelected(GameType::Shortcut(
                            shortcut.clone(),
                        )));
                    }
                }
            }
            None => {
                ui.label("Could not find any shortcuts");
            }
        }
        None
    }

    fn render_possible_images(
        &self,
        ui: &mut egui::Ui,
        image_type: &ImageType,
        state: &ImageSelectState,
    ) -> Option<UserAction> {
        ui.heading(image_type.name());

        if ui
            .small_button("Clear image?")
            .on_hover_text("Click here to clear the image")
            .clicked()
        {
            return Some(UserAction::ImageTypeCleared(image_type.clone(), false));
        }

        if ui
            .small_button("Stop downloading this image?")
            .on_hover_text("Stop downloading this type of image for this shortcut at all")
            .clicked()
        {
            return Some(UserAction::ImageTypeCleared(image_type.clone(), true));
        }
        match &*state.image_options.borrow() {
            FetcStatus::Fetched(images) => {
                for image in images {
                    let image_key = image.thumbnail_path.as_path().to_string_lossy().to_string();

                    match state.image_handles.get_mut(&image_key) {
                        Some(mut state) => {
                            match state.value() {
                                TextureState::Downloading => {
                                    ui.ctx().request_repaint();
                                    //nothing to do,just wait
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label(format!("Downloading id {}", image.id));
                                    });
                                }
                                TextureState::Downloaded => {
                                    //Need to load
                                    let image_data = load_image_from_path(&image.thumbnail_path);
                                    if let Some(image_data) = image_data {
                                        let handle = ui.ctx().load_texture(&image_key, image_data);
                                        *state.value_mut() = TextureState::Loaded(handle);
                                    }
                                    ui.ctx().request_repaint();
                                    ui.horizontal(|ui| {
                                        ui.spinner();
                                        ui.label("Loading");
                                    });
                                }
                                TextureState::Loaded(texture_handle) => {
                                    //need to show
                                    let mut size = texture_handle.size_vec2();
                                    clamp_to_width(&mut size, MAX_WIDTH);
                                    let image_button = ImageButton::new(texture_handle, size);
                                    if ui.add(image_button).clicked() {
                                        return Some(UserAction::ImageSelected(image.clone()));
                                    }
                                }
                            }
                        }
                        None => {
                            //We need to start a download
                            let image_handles = &self.image_selected_state.image_handles;
                            let path = &image.thumbnail_path;
                            //Redownload if file is too small
                            if !path.exists()
                                || std::fs::metadata(path).map(|m| m.len()).unwrap_or_default() < 2
                            {
                                image_handles.insert(image_key.clone(), TextureState::Downloading);
                                let to_download = ToDownload {
                                    path: path.clone(),
                                    url: image.thumbnail_url.clone(),
                                    app_name: "Thumbnail".to_string(),
                                    image_type: *image_type,
                                };
                                let image_handles = image_handles.clone();
                                let image_key = image_key.clone();
                                self.rt.spawn_blocking(move || {
                                    block_on(crate::steamgriddb::download_to_download(
                                        &to_download,
                                    ))
                                    .unwrap();
                                    image_handles.insert(image_key, TextureState::Downloaded);
                                });
                            } else {
                                image_handles.insert(image_key.clone(), TextureState::Downloaded);
                            }
                        }
                    }
                }
            }
            _ => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Finding possible images");
                });
                ui.ctx().request_repaint();
            }
        }
        None
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
                self.handle_user_selected(user, ui);
            }
            UserAction::ShortcutSelected(shortcut) => {
                self.handle_shortcut_selected(shortcut, ui);
            }
            UserAction::ImageTypeSelected(image_type) => {
                self.handle_image_type_selected(image_type);
            }
            UserAction::ImageSelected(image) => {
                self.handle_image_selected(image);
            }
            UserAction::BackButton => {
                self.handle_back_button_action();
            }
            UserAction::GridIdChanged(grid_id) => {
                self.handle_grid_change(grid_id);
            }
            UserAction::SetGamesMode(game_mode) => {
                self.handle_set_game_mode(game_mode);
            }
            UserAction::NoAction => {}
            UserAction::CorrectGridId => {
                self.handle_correct_grid_request();
            }
            UserAction::ImageTypeCleared(image_type, should_ban) => {
                let app_id = self
                    .image_selected_state
                    .selected_shortcut
                    .as_ref()
                    .unwrap()
                    .app_id();
                self.settings
                    .steamgrid_db
                    .set_image_banned(&image_type, app_id, should_ban);
                self.handle_image_type_clear(image_type);
            }
        };
    }

    fn handle_image_type_clear(&mut self, image_type: ImageType) {
        let data_folder = &self
            .image_selected_state
            .steam_user
            .as_ref()
            .unwrap()
            .steam_user_data_folder;
        for ext in POSSIBLE_EXTENSIONS {
            let file_name = image_type.file_name(
                self.image_selected_state
                    .selected_shortcut
                    .as_ref()
                    .unwrap()
                    .app_id(),
                ext,
            );
            let path = Path::new(data_folder)
                .join("config")
                .join("grid")
                .join(&file_name);
            if path.exists() {
                let _ = std::fs::remove_file(&path);
            }
            let key = path.to_string_lossy().to_string();
            self.image_selected_state.image_handles.remove(&key);
        }
        self.image_selected_state.image_type_selected = None;
    }

    fn handle_correct_grid_request(&mut self) {
        let app_name = self
            .image_selected_state
            .selected_shortcut
            .as_ref()
            .map(|s| s.name().clone())
            .unwrap_or_default();
        let auth_key = self
            .settings
            .steamgrid_db
            .auth_key
            .clone()
            .unwrap_or_default();
        let client = steamgriddb_api::Client::new(&auth_key);
        let search_results = self.rt.block_on(client.search(&app_name));
        self.image_selected_state.possible_names = search_results.ok();
    }

    fn handle_set_game_mode(&mut self, game_mode: GameMode) {
        self.image_selected_state.game_mode = game_mode;
        self.image_selected_state.steam_games = Some(get_installed_games(&self.settings.steam));
    }
    fn handle_grid_change(&mut self, grid_id: usize) {
        self.image_selected_state.grid_id = Some(grid_id);
        self.image_selected_state.possible_names = None;
        if let Some(auth_key) = &self.settings.steamgrid_db.auth_key {
            let client = steamgriddb_api::Client::new(auth_key);
            let mut cache = CachedSearch::new(&client);
            if let Some(shortcut) = &self.image_selected_state.selected_shortcut {
                cache.set_cache(shortcut.app_id(), shortcut.name().clone(), grid_id);
                cache.save();
            }
        }
    }

    fn handle_user_selected(&mut self, user: SteamUsersInfo, ui: &mut egui::Ui) {
        let state = &mut self.image_selected_state;
        let user_info = crate::steam::get_shortcuts_for_user(&user);
        let mut user_folder = user_info.path.clone();
        user_folder.pop();
        user_folder.pop();
        let mut shortcuts = user_info.shortcuts;
        shortcuts.sort_by_key(|s| s.app_name.clone());

        let image_type = &ImageType::Grid;
        for shortcut in &shortcuts {
            let (path, key) = shortcut.key(image_type, &user_folder);
            let loaded = state.image_handles.contains_key(&key);
            if !loaded && path.exists() {
                let image = load_image_from_path(&path);
                if let Some(image) = image {
                    let texture = ui.ctx().load_texture(&key, image);
                    state
                        .image_handles
                        .insert(key, TextureState::Loaded(texture));
                }
            }
        }

        state.user_shortcuts = Some(shortcuts);
        state.steam_user = Some(user);
    }

    fn handle_image_type_selected(&mut self, image_type: ImageType) {
        let state = &mut self.image_selected_state;
        state.image_type_selected = Some(image_type);
        let (tx, rx) = watch::channel(FetcStatus::Fetching);
        self.image_selected_state.image_options = rx;
        let settings = self.settings.clone();
        if let Some(auth_key) = settings.steamgrid_db.auth_key {
            if let Some(grid_id) = self.image_selected_state.grid_id {
                let auth_key = auth_key;
                let image_type = image_type;
                self.rt.spawn_blocking(move || {
                    let thumbnails_folder = get_thumbnails_folder();
                    let client = steamgriddb_api::Client::new(auth_key);
                    let query = get_query_type(false, &image_type);
                    let search_res = block_on(client.get_images_for_id(grid_id, &query));
                    if let Ok(possible_images) = search_res {
                        let mut result = vec![];
                        for possible_image in &possible_images {
                            let path = thumbnails_folder.join(format!("{}.png", possible_image.id));
                            result.push(PossibleImage {
                                thumbnail_path: path,
                                mime: possible_image.mime.clone(),
                                thumbnail_url: possible_image.thumb.clone(),
                                full_url: possible_image.url.clone(),
                                id: possible_image.id,
                            });
                            let _ = tx.send(FetcStatus::Fetched(result.clone()));
                        }
                    }
                });
            }
        };
    }

    fn handle_image_selected(&mut self, image: PossibleImage) {
        //We must have a user here
        let user = self.image_selected_state.steam_user.as_ref().unwrap();
        let selected_image_type = self
            .image_selected_state
            .image_type_selected
            .as_ref()
            .unwrap();
        let selected_image = self
            .image_selected_state
            .selected_shortcut
            .as_ref()
            .unwrap();

        let ext = get_image_extension(&image.mime);
        let to = Path::new(&user.steam_user_data_folder)
            .join("config")
            .join("grid")
            .join(selected_image_type.file_name(selected_image.app_id(), ext));

        if to.exists() {
            let old_key = to.to_string_lossy().to_string();
            let new_key = image.thumbnail_path.to_string_lossy().to_string();
            let _ = self.image_selected_state.image_handles.remove(&old_key);
            let swap = self.image_selected_state.image_handles.get(&new_key);
            if let Some(swp) = swap {
                self.image_selected_state
                    .image_handles
                    .insert(old_key, swp.value().clone());
            }
        }

        let app_name = selected_image.name();
        let to_download = ToDownload {
            path: to,
            url: image.full_url.clone(),
            app_name: app_name.to_string(),
            image_type: *selected_image_type,
        };
        self.rt.spawn_blocking(move || {
            let _ = block_on(crate::steamgriddb::download_to_download(&to_download));
        });

        self.clear_loaded_images();
        {
            self.image_selected_state.image_type_selected = None;
            self.image_selected_state.image_options = watch::channel(FetcStatus::NeedsFetched).1;
        }
    }

    fn clear_loaded_images(&mut self) {
        match &*self.image_selected_state.image_options.borrow() {
            FetcStatus::Fetched(options) => {
                for option in options {
                    let key = option.thumbnail_path.to_string_lossy().to_string();
                    self.image_selected_state.image_handles.remove(&key);
                }
            }
            _ => {}
        }
    }

    fn handle_shortcut_selected(&mut self, shortcut: GameType, ui: &mut egui::Ui) {
        let state = &mut self.image_selected_state;
        //We must have a user to make see this action;
        let user = state.steam_user.as_ref().unwrap();
        if let Some(auth_key) = &self.settings.steamgrid_db.auth_key {
            let client = steamgriddb_api::Client::new(auth_key);
            let search = CachedSearch::new(&client);
            state.grid_id = self
                .rt
                .block_on(search.search(shortcut.app_id(), shortcut.name()))
                .ok()
                .flatten();
        }
        state.selected_shortcut = Some(shortcut.clone());

        for image_type in ImageType::all() {
            let (path, key) = shortcut.key(image_type, &Path::new(&user.steam_user_data_folder));
            let image = load_image_from_path(&path);
            if let Some(image) = image {
                let texture = ui.ctx().load_texture(&key, image);
                state
                    .image_handles
                    .insert(key, TextureState::Loaded(texture));
            }
        }
        state.selected_shortcut = Some(shortcut);
    }

    fn handle_back_button_action(&mut self) {
        let state = &mut self.image_selected_state;
        if state.possible_names.is_some() {
            state.possible_names = None;
        } else if state.image_type_selected.is_some() {
            state.image_type_selected = None;
        } else if state.selected_shortcut.is_some() {
            state.selected_shortcut = None;
        } else {
            state.image_handles.clear();
            state.user_shortcuts = None;
            state.steam_user = None;
        }
    }
}

fn render_possible_names(
    possible_names: &Vec<steamgriddb_api::search::SearchResult>,
    ui: &mut egui::Ui,
) -> Option<UserAction> {
    for possible in possible_names {
        if ui.button(&possible.name).clicked() {
            return Some(UserAction::GridIdChanged(possible.id));
        }
    }
    None
}

fn render_steam_game_select(ui: &mut egui::Ui, state: &ImageSelectState) -> Option<UserAction> {
    if let Some(steam_games) = state.steam_games.as_ref() {
        for game in steam_games {
            if ui.button(&game.name).clicked() {
                return Some(UserAction::ShortcutSelected(GameType::SteamGame(
                    game.clone(),
                )));
            }
        }
    }
    None
}

fn render_shortcut_images(ui: &mut egui::Ui, state: &ImageSelectState) -> Option<UserAction> {
    let mut grid_id_text = state.grid_id.map(|id| id.to_string()).unwrap_or_default();
    if ui.text_edit_singleline(&mut grid_id_text).changed() {
        if let Ok(grid_id) = grid_id_text.parse::<usize>() {
            return Some(UserAction::GridIdChanged(grid_id));
        }
    };
    if ui
        .button("Click here if the images are for a wrong game")
        .clicked()
    {
        return Some(UserAction::CorrectGridId);
    }

    let shortcut = state.selected_shortcut.as_ref().unwrap();
    let user_path = &state.steam_user.as_ref().unwrap().steam_user_data_folder;
    for image_type in ImageType::all() {
        ui.label(image_type.name());
        let (_path, key) = shortcut.key(&image_type, Path::new(&user_path));
        let texture = state
            .image_handles
            .get(&key)
            .map(|k| match k.value() {
                TextureState::Loaded(texture) => Some(texture.clone()),
                _ => None,
            })
            .flatten();
        let clicked = render_thumbnail(ui, texture);
        if clicked {
            return Some(UserAction::ImageTypeSelected(*image_type));
        }
    }
    None
}

fn render_user_select(state: &ImageSelectState, ui: &mut egui::Ui) -> UserAction {
    let users = state.steam_users.as_ref().unwrap();
    if users.len() == 1 {
        return UserAction::UserSelected(users[0].clone());
    }
    for user in users {
        if ui.button(&user.user_id).clicked() {
            return UserAction::UserSelected(user.clone());
        }
    }
    UserAction::NoAction
}

const MAX_WIDTH: f32 = 300.;

fn render_thumbnail(ui: &mut egui::Ui, image: Option<egui::TextureHandle>) -> bool {
    if let Some(texture) = image {
        let mut size = texture.size_vec2();
        clamp_to_width(&mut size, MAX_WIDTH);
        let image_button = ImageButton::new(&texture, size);
        let added = ui.add(image_button);
        added.on_hover_text("Click to change image").clicked()
    } else {
        ui.button("Pick an image").clicked()
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

trait HasImageKey {
    fn key(&self, image_type: &ImageType, user_path: &Path) -> (PathBuf, String);
}
const POSSIBLE_EXTENSIONS: [&'static str; 4] = ["png", "jpg", "ico", "webp"];

impl HasImageKey for GameType {
    fn key(&self, image_type: &ImageType, user_path: &Path) -> (PathBuf, String) {
        match self {
            GameType::Shortcut(s) => s.key(image_type, user_path),
            GameType::SteamGame(g) => g.key(image_type, user_path),
        }
    }
}
impl HasImageKey for SteamGameInfo {
    fn key(&self, image_type: &ImageType, user_path: &Path) -> (PathBuf, String) {
        let mut keys = POSSIBLE_EXTENSIONS
            .iter()
            .map(|ext| key_from_extension(self.appid, image_type, user_path, ext));
        let first = keys.next().unwrap();
        let other = keys.filter(|(exsists, _, _)| *exsists).next();
        let (_, path, key) = other.unwrap_or(first);
        (path, key)
    }
}

impl HasImageKey for ShortcutOwned {
    fn key(&self, image_type: &ImageType, user_path: &Path) -> (PathBuf, String) {
        let mut keys = POSSIBLE_EXTENSIONS
            .iter()
            .map(|ext| key_from_extension(self.app_id, image_type, user_path, ext));
        let first = keys.next().unwrap();
        let other = keys.filter(|(exsists, _, _)| *exsists).next();
        let (_, path, key) = other.unwrap_or(first);
        (path, key)
    }
}

fn key_from_extension(
    app_id: u32,
    image_type: &ImageType,
    user_path: &Path,
    ext: &str,
) -> (bool, PathBuf, String) {
    let file_name = image_type.file_name(app_id, ext);
    let path = user_path.join("config").join("grid").join(&file_name);
    let key = path.to_string_lossy().to_string();
    (path.exists(), path, key)
}

use super::{
    constants::POSSIBLE_EXTENSIONS,
    gamemode::GameMode,
    gametype::GameType,
    hasimagekey::HasImageKey,
    image_select_state::ImageSelectState,
    pages::{
        render_page_pick_image, render_page_shortcut_images_overview,
        render_page_shortcut_select_image_type, render_page_steam_images_overview, handle_grid_change,
    },
    possible_image::PossibleImage,
    texturestate::TextureDownloadState,
    useraction::UserAction,
};

use std::path::Path;

    use crate::{
    config::get_thumbnails_folder,
    steam::get_shortcuts_paths,
    steam::{get_installed_games, SteamUsersInfo},
    steamgriddb::{get_image_extension, get_query_type, CachedSearch, ImageType, ToDownload},
    sync::{download_images, SyncProgress},
    ui::{ui_images::load_image_from_path, FetcStatus, MyEguiApp},
};
use egui::ScrollArea;
use futures::executor::block_on;
use steam_shortcuts_util::shortcut::ShortcutOwned;
use tokio::sync::watch;

impl MyEguiApp {
    fn render_ui_image_action(&self, ui: &mut egui::Ui) -> UserAction {
        let state = &self.image_selected_state;
        if let Some(action) = ui
            .horizontal(|ui| {
                let render_back =
                    state.selected_shortcut.is_some() || state.image_type_selected.is_some();
                if render_back {
                    if ui.button("Back").clicked() {
                        return Some(UserAction::BackButton);
                    }
                    None
                } else {
                    if let Some(value) = render_user_select(state, ui) {
                        return Some(value);
                    }
                    render_shortcut_mode_select(state, ui)
                }
            })
            .inner
        {
            return action;
        }

        if let Some(shortcut) = state.selected_shortcut.as_ref() {
            ui.heading(shortcut.name());

            if let Some(possible_names) = state.possible_names.as_ref() {
                if let Some(value) =
                    super::pages::render_page_change_grid_db_id(possible_names, ui, state)
                {
                    return value;
                }
            } else if let Some(image_type) = state.image_type_selected.as_ref() {
                if let Some(action) = render_page_pick_image(&self, ui, image_type, state) {
                    return action;
                }
            } else if let Some(action) = render_page_shortcut_select_image_type(ui, state) {
                return action;
            }
        } else {
            let is_shortcut = state.game_mode.is_shortcuts();
            if is_shortcut {
                if let Some(action) = render_page_shortcut_images_overview(&self, ui) {
                    return action;
                }
            } else if let Some(action) = render_page_steam_images_overview(ui, state) {
                return action;
            }

            if let Some(value) = self.render_find_all_images(ui) {
                return value;
            }
        }

        UserAction::NoAction
    }

    fn render_find_all_images(&self, ui: &mut egui::Ui) -> Option<UserAction> {
        match *self.status_reciever.borrow() {
            crate::sync::SyncProgress::FindingImages => {
                ui.spinner();
                ui.label("Finding images to download");
                ui.ctx().request_repaint();
            }
            crate::sync::SyncProgress::DownloadingImages { to_download } => {
                ui.spinner();
                ui.label(format!("Downloading {to_download} images"));
                ui.ctx().request_repaint();
            }
            crate::sync::SyncProgress::Done => {
                ui.ctx().request_repaint();
                return Some(UserAction::RefreshImages);
            }
            _ => {
                if ui.button("Download images for all games").clicked() {
                    return Some(UserAction::DownloadAllImages);
                }
            }
        }
        None
    }

    fn ensure_steam_users_loaded(&mut self) {
        if self.image_selected_state.settings_error.is_none()
            && self.image_selected_state.steam_users.is_none()
        {
            let paths = get_shortcuts_paths(&self.settings.steam);
            match paths {
                Ok(paths) => self.image_selected_state.steam_users = Some(paths),
                Err(err) => {
                    self.image_selected_state.settings_error = Some(format!("Could not find user steam location, error message: {} , try to clear the steam location field in settings to let BoilR find it itself",err));
                }
            }
        }
    }

    pub fn render_ui_images(&mut self, ui: &mut egui::Ui) {
        self.ensure_steam_users_loaded();

        if let Some(error_message) = &self.image_selected_state.settings_error {
            ui.label(error_message);
            return;
        }

        let mut action = UserAction::NoAction;
        ScrollArea::vertical()
            .stick_to_right(true)
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
                handle_grid_change(self,grid_id);
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
            UserAction::ClearImages => {
                for image_type in ImageType::all() {
                    self.handle_image_type_clear(*image_type);
                }
                self.handle_back_button_action();
            }
            UserAction::DownloadAllImages => {
                if let Some(users) = &self.image_selected_state.steam_users {
                    let (sender, reciever) = watch::channel(SyncProgress::FindingImages);
                    self.status_reciever = reciever;
                    let mut sender_op = Some(sender);
                    let settings = self.settings.clone();
                    let users = users.clone();
                    self.rt.spawn_blocking(move || {
                        let task = download_images(&settings, &users, &mut sender_op);
                        block_on(task);
                        let _ = sender_op.unwrap().send(SyncProgress::Done);
                    });
                }
            }
            UserAction::RefreshImages => {
                let (_, reciever) = watch::channel(SyncProgress::NotStarted);
                let user = self.image_selected_state.steam_user.clone();
                if let Some(user) = &user {
                    load_image_grids(user, &mut self.image_selected_state, ui);
                }
                self.status_reciever = reciever;
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
            .map(|s| s.name())
            .unwrap_or_default();
        let auth_key = self
            .settings
            .steamgrid_db
            .auth_key
            .clone()
            .unwrap_or_default();
        let client = steamgriddb_api::Client::new(&auth_key);
        let search_results = self.rt.block_on(client.search(app_name));
        self.image_selected_state.possible_names = search_results.ok();
    }

    fn handle_set_game_mode(&mut self, game_mode: GameMode) {
        self.image_selected_state.game_mode = game_mode;
        self.image_selected_state.steam_games = Some(get_installed_games(&self.settings.steam));
    }

    fn handle_user_selected(&mut self, user: SteamUsersInfo, ui: &mut egui::Ui) {
        let state = &mut self.image_selected_state;
        let shortcuts = load_image_grids(&user, state, ui);
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
                    let query =
                        get_query_type(false, &image_type, settings.steamgrid_db.allow_nsfw);
                    let search_res = block_on(client.get_images_for_id(grid_id, &query));
                    if let Ok(possible_images) = search_res {
                        let mut result = vec![];
                        for possible_image in &possible_images {
                            let ext = get_image_extension(&possible_image.mime);
                            let path =
                                thumbnails_folder.join(format!("{}.{}", possible_image.id, ext));
                            result.push(PossibleImage {
                                thumbnail_path: path,
                                mime: possible_image.mime.clone(),
                                thumbnail_url: possible_image.thumb.clone(),
                                full_url: possible_image.url.clone(),
                            });
                        }
                        let _ = tx.send(FetcStatus::Fetched(result));
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
        let selected_shortcut = self
            .image_selected_state
            .selected_shortcut
            .as_ref()
            .unwrap();

        let ext = get_image_extension(&image.mime);
        let to_download_to_path = Path::new(&user.steam_user_data_folder)
            .join("config")
            .join("grid")
            .join(selected_image_type.file_name(selected_shortcut.app_id(), ext));

        //Delete old possible images

        let data_folder = Path::new(&user.steam_user_data_folder);

        //Keep deleting images of this type untill we don't find any more
        let mut path = self.get_shortcut_image_path(data_folder);
        while Path::new(&path).exists() {
            let _ = std::fs::remove_file(&path);
            path = self.get_shortcut_image_path(data_folder);
        }

        //Put the loaded thumbnail into the image handler map, we can use that for preview
        let full_image_key = to_download_to_path.to_string_lossy().to_string();
        let _ = self
            .image_selected_state
            .image_handles
            .remove(&full_image_key);
        let thumbnail_key = image.thumbnail_path.to_string_lossy().to_string();
        let thumbnail = self
            .image_selected_state
            .image_handles
            .remove(&thumbnail_key);
        if let Some((_key, thumbnail)) = thumbnail {
            self.image_selected_state
                .image_handles
                .insert(full_image_key, thumbnail);
        }

        let app_name = selected_shortcut.name();
        let to_download = ToDownload {
            path: to_download_to_path,
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

    fn get_shortcut_image_path(&self, data_folder: &Path) -> String {
        self.image_selected_state
            .selected_shortcut
            .as_ref()
            .unwrap()
            .key(
                &self.image_selected_state.image_type_selected.unwrap(),
                data_folder,
            )
            .1
    }

    fn clear_loaded_images(&mut self) {
        if let FetcStatus::Fetched(options) = &*self.image_selected_state.image_options.borrow() {
            for option in options {
                let key = option.thumbnail_path.to_string_lossy().to_string();
                self.image_selected_state.image_handles.remove(&key);
            }
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
            let (path, key) = shortcut.key(image_type, Path::new(&user.steam_user_data_folder));
            let image = load_image_from_path(&path);
            if let Ok(image) = image {
                let texture = ui
                    .ctx()
                    .load_texture(&key, image, egui::TextureOptions::LINEAR);
                state
                    .image_handles
                    .insert(key, TextureDownloadState::Loaded(texture));
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

fn load_image_grids(
    user: &SteamUsersInfo,
    state: &mut ImageSelectState,
    ui: &mut egui::Ui,
) -> Vec<ShortcutOwned> {
    let user_info = crate::steam::get_shortcuts_for_user(user);
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
            if let Ok(image) = image {
                let texture = ui
                    .ctx()
                    .load_texture(&key, image, egui::TextureOptions::LINEAR);
                state
                    .image_handles
                    .insert(key, TextureDownloadState::Loaded(texture));
            }
        }
    }
    shortcuts
}

fn render_shortcut_mode_select(state: &ImageSelectState, ui: &mut egui::Ui) -> Option<UserAction> {
    let mode_before = state.game_mode.clone();
    let combo_box = egui::ComboBox::new("ImageModeSelect", "").selected_text(mode_before.label());
    let mut mode_after = state.game_mode.clone();
    combo_box.show_ui(ui, |ui| {
        ui.selectable_value(
            &mut mode_after,
            GameMode::Shortcuts,
            GameMode::Shortcuts.label(),
        );
        ui.selectable_value(
            &mut mode_after,
            GameMode::SteamGames,
            GameMode::SteamGames.label(),
        );
    });
    if !mode_after.eq(&mode_before) {
        return Some(UserAction::SetGamesMode(mode_after));
    }
    None
}

fn render_user_select(state: &ImageSelectState, ui: &mut egui::Ui) -> Option<UserAction> {
    if state.steam_user.is_none() {
        if let Some(users) = &state.steam_users {
            if users.len() > 0 {
                return Some(UserAction::UserSelected(users[0].clone()));
            }
        }
    } else {
        let mut selected_user = state.steam_user.as_ref().unwrap().clone();
        let id_before = selected_user.user_id.clone();
        if let Some(steam_users) = &state.steam_users {
            if steam_users.len() > 0 {
                let combo_box = egui::ComboBox::new("ImageUserSelect", "")
                    .selected_text(format!("Steam user id: {}", &selected_user.user_id));
                combo_box.show_ui(ui, |ui| {
                    for user in steam_users {
                        ui.selectable_value(&mut selected_user, user.clone(), &user.user_id);
                    }
                });
            }
        }
        let id_now = selected_user.user_id.clone();
        if !id_before.eq(&id_now) {
            return Some(UserAction::UserSelected(selected_user.clone()));
        }
    }

    None
}

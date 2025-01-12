use super::{
    constants::POSSIBLE_EXTENSIONS,
    gamemode::GameMode,
    image_select_state::ImageSelectState,
    pages::{
        handle_correct_grid_request, handle_grid_change, handle_image_selected,
        handle_shortcut_selected, render_page_pick_image, render_page_shortcut_images_overview,
        render_page_shortcut_select_image_type, render_page_steam_images_overview,
    },
    possible_image::PossibleImage,
    useraction::UserAction,
};

use std::{ path::Path, thread, time::Duration};

use crate::{
    config::get_thumbnails_folder,
    steam::get_shortcuts_paths,
    steam::{get_installed_games, SteamUsersInfo},
    steamgriddb::{get_image_extension, get_query_type, ImageType},
    sync::{download_images, SyncProgress},
    ui::{components::render_user_select, FetchStatus, MyEguiApp},
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
                    let users = state.steam_users.as_ref();
                    if let Some(users) = users {
                        if let Some(value) =
                            render_user_select(state.steam_user.as_ref(), users, ui)
                        {
                            return Some(UserAction::UserSelected(value.clone()));
                        }
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
                if let Some(action) = render_page_pick_image(ui, image_type, state) {
                    return action;
                }
            } else if let Some(action) = render_page_shortcut_select_image_type(ui, state) {
                return action;
            }
        } else {
            let is_shortcut = state.game_mode.is_shortcuts();
            if is_shortcut {
                if let Some(action) = render_page_shortcut_images_overview(self, ui) {
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
                    self.image_selected_state.settings_error = Some(format!("Could not find user steam location, error message: {err} , try to clear the steam location field in settings to let BoilR find it itself"));
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
                self.handle_user_selected(user);
            }
            UserAction::ShortcutSelected(shortcut) => {
                handle_shortcut_selected(self, shortcut);
            }
            UserAction::ImageTypeSelected(image_type) => {
                self.handle_image_type_selected(image_type);
            }
            UserAction::ImageSelected(image) => {
                handle_image_selected(self, image);
                thread::sleep(Duration::from_millis(100));
                ui.ctx().forget_all_images();
            }
            UserAction::BackButton => {
                self.handle_back_button_action();
            }
            UserAction::GridIdChanged(grid_id) => {
                handle_grid_change(self, grid_id);
            }
            UserAction::SetGamesMode(game_mode) => {
                self.handle_set_game_mode(game_mode);
            }
            UserAction::NoAction => {}
            UserAction::CorrectGridId => {
                handle_correct_grid_request(self);
            }
            UserAction::ImageTypeCleared(image_type, should_ban) => {
                self.handle_image_type_cleared(image_type, should_ban);
                ui.ctx().forget_all_images();
            }
            UserAction::ClearImages => {
                self.handle_clear_all_images();
                ui.ctx().forget_all_images();
            }
            UserAction::DownloadAllImages => {
                self.handle_download_all_images();
                ui.ctx().forget_all_images();
            }
            UserAction::RefreshImages => {
                let user = self.image_selected_state.steam_user.clone();
                if let Some(user) = &user {
                    load_image_grids(user);
                }
                ui.ctx().forget_all_images();
            }
        };
    }

    fn handle_image_type_cleared(&mut self, image_type: ImageType, should_ban: bool) {
        let app_id = self
            .image_selected_state
            .selected_shortcut
            .as_ref()
            .map(|m| m.app_id());
        if let Some(app_id) = app_id {
            self.settings
                .steamgrid_db
                .set_image_banned(&image_type, app_id, should_ban);
        }

        self.handle_image_type_clear(image_type);
    }

    fn handle_clear_all_images(&mut self) {
        for image_type in ImageType::all() {
            self.handle_image_type_clear(*image_type);
        }
        self.handle_back_button_action();
    }

    fn handle_download_all_images(&mut self) {
        if let Some(users) = &self.image_selected_state.steam_users {
            let (sender, reciever) = watch::channel(SyncProgress::FindingImages);
            self.status_reciever = reciever;
            let mut sender_op = Some(sender);
            let settings = self.settings.clone();
            let users = users.clone();
            self.rt.spawn_blocking(move || {
                let task = download_images(&settings, &users, &mut sender_op);
                block_on(task);
                if let Some(sender_op) = sender_op {
                    let _ = sender_op.send(SyncProgress::Done);
                }
            });
        }
    }

    fn handle_image_type_clear(&mut self, image_type: ImageType) {
        let app_id = self
            .image_selected_state
            .selected_shortcut
            .as_ref()
            .map(|s| s.app_id());
        let data_folder = &self
            .image_selected_state
            .steam_user
            .as_ref()
            .map(|s| &s.steam_user_data_folder);
        if let (Some(app_id), Some(data_folder)) = (app_id, data_folder) {
            for ext in POSSIBLE_EXTENSIONS {
                let file_name = image_type.file_name(app_id, ext);
                let path = Path::new(data_folder)
                    .join("config")
                    .join("grid")
                    .join(file_name);
                if path.exists() {
                    let _ = std::fs::remove_file(&path);
                }
            }
            self.image_selected_state.image_type_selected = None;
        }
    }

    fn handle_set_game_mode(&mut self, game_mode: GameMode) {
        self.image_selected_state.game_mode = game_mode;
        self.image_selected_state.steam_games = Some(get_installed_games(&self.settings.steam));
    }

    fn handle_user_selected(&mut self, user: SteamUsersInfo) {
        let state = &mut self.image_selected_state;
        let shortcuts = load_image_grids(&user);
        state.user_shortcuts = Some(shortcuts);
        state.steam_user = Some(user);
    }

    fn handle_image_type_selected(&mut self, image_type: ImageType) {
        let state = &mut self.image_selected_state;
        state.image_type_selected = Some(image_type);
        let (tx, rx) = watch::channel(FetchStatus::Fetching);
        self.image_selected_state.image_options = rx;
        let settings = self.settings.clone();
        if let Some(auth_key) = settings.steamgrid_db.auth_key {
            if let Some(grid_id) = self.image_selected_state.grid_id {
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
                        let _ = tx.send(FetchStatus::Fetched(result));
                    }
                });
            }
        };
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
            state.user_shortcuts = None;
            state.steam_user = None;
        }
    }
}

//TODO remove this
fn load_image_grids(user: &SteamUsersInfo) -> Vec<ShortcutOwned> {
    let user_info = crate::steam::get_shortcuts_for_user(user);
    match user_info {
        Ok(user_info) => {
            let mut user_folder = user_info.path.clone();
            user_folder.pop();
            user_folder.pop();
            let mut shortcuts = user_info.shortcuts;
            shortcuts.sort_by_key(|s| s.app_name.clone());
            shortcuts
        }
        Err(_err) => {
            vec![]
        }
    }
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

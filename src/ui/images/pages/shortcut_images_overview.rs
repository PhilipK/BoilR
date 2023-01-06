use std::path::Path;

use steam_shortcuts_util::shortcut::ShortcutOwned;

use crate::{
    steam::SteamUsersInfo,
    steamgriddb::{CachedSearch, ImageType},
    ui::{
        components::render_image_from_path,
        images::{
            gametype::GameType, hasimagekey::HasImageKey, texturestate::TextureDownloadState,
            useraction::UserAction,
        },
        ui_images::load_image_from_path,
        MyEguiApp,
    },
};

pub fn render_page_shortcut_images_overview(
    app: &MyEguiApp,
    ui: &mut egui::Ui,
) -> Option<UserAction> {
    let shortcuts = &app.image_selected_state.user_shortcuts;

    let width = ui.available_size().x;
    let column_width = 100.;
    let column_padding = 23.;
    let columns = (width / (column_width + column_padding)).floor() as u32;
    let mut cur_column = 0;
    match shortcuts {
        Some(shortcuts) => {
            let user_info = &app.image_selected_state.steam_user.as_ref().unwrap();
            if let Some(action) = egui::Grid::new("ui_images")
                .show(ui, |ui| {
                    for shortcut in shortcuts {
                        let action = render_image(app, shortcut, user_info, column_width, ui);
                        if action.is_some() {
                            return action;
                        }
                        cur_column += 1;
                        if cur_column >= columns {
                            cur_column = 0;
                            ui.end_row();
                        }
                    }
                    ui.end_row();
                    None
                })
                .inner
            {
                return action;
            }
        }
        None => {
            ui.label("Could not find any shortcuts");
        }
    }
    None
}

fn render_image(
    app: &MyEguiApp,
    shortcut: &ShortcutOwned,
    user_info: &&SteamUsersInfo,
    column_width: f32,
    ui: &mut egui::Ui,
) -> Option<Option<UserAction>> {
    let (_, key) = shortcut.key(
        &ImageType::Grid,
        Path::new(&user_info.steam_user_data_folder),
    );

    let clicked = render_image_from_path(
        ui,
        &app.image_selected_state.image_handles,
        Path::new(&key),
        column_width,
        &shortcut.app_name,
    );
    if clicked {
        return Some(Some(UserAction::ShortcutSelected(GameType::Shortcut(
            shortcut.clone(),
        ))));
    }
    None
}
pub fn handle_shortcut_selected(app: &mut MyEguiApp, shortcut: GameType, ui: &mut egui::Ui) {
    let state = &mut app.image_selected_state;
    //We must have a user to make see this action;
    if let Some(user) = state.steam_user.as_ref() {
        if let Some(auth_key) = &app.settings.steamgrid_db.auth_key {
            let client = steamgriddb_api::Client::new(auth_key);
            let search = CachedSearch::new(&client);
            state.grid_id = app
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
}

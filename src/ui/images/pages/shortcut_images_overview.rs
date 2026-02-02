use std::path::Path;

use egui::ImageButton;
use steam_shortcuts_util::shortcut::ShortcutOwned;
use tokio::sync::watch;

use crate::{
    steam::SteamUsersInfo,
    steamgriddb::{CachedSearch, ImageType},
    ui::{
        images::{
            gametype::GameType, hasimagekey::HasImageKey,
            useraction::UserAction,
        },
        FetchStatus, MyEguiApp,
    },
};

pub fn render_page_shortcut_images_overview(
    app: &MyEguiApp,
    ui: &mut egui::Ui,
) -> Option<UserAction> {
    let user_info = &app.image_selected_state.steam_user;
    let shortcuts = &app.image_selected_state.user_shortcuts;
    let width = ui.available_size().x;
    let column_width = 100.;
    let column_padding = 23.;
    let columns = (width / (column_width + column_padding)).floor() as u32;
    let mut cur_column = 0;
    match (user_info, shortcuts) {
        (Some(user_info), Some(shortcuts)) => {
            if let Some(action) = egui::Grid::new("ui_images")
                .show(ui, |ui| {
                    for shortcut in shortcuts {
                        let action = render_image(shortcut, user_info, column_width, ui);
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
        _ => {
            ui.label("Could not find any shortcuts");
        }
    }
    None
}

fn render_image(
    shortcut: &ShortcutOwned,
    user_info: &SteamUsersInfo,
    column_width: f32,
    ui: &mut egui::Ui,
) -> Option<Option<UserAction>> {
    let (_, key) = shortcut.key(
        &ImageType::Grid,
        Path::new(&user_info.steam_user_data_folder),
    );
    // Convert Windows backslashes to forward slashes for file:// URL
    let key_normalized = key.replace('\\', "/");
    let image = egui::Image::new(format!("file:///{}", key_normalized)).max_width(column_width).shrink_to_fit();
    let calced = image.calc_size(egui::Vec2 { x: column_width, y: f32::INFINITY }, image.size());
    let button = ImageButton::new(image);

    if ui.add_sized(calced,button).on_hover_text(&shortcut.app_name).clicked() {
        return Some(Some(UserAction::ShortcutSelected(GameType::Shortcut(
            Box::new(shortcut.clone()),
        ))));
    }
    None
}
pub fn handle_shortcut_selected(app: &mut MyEguiApp, shortcut: GameType) {
    // Set the selected shortcut immediately so UI can show it
    app.image_selected_state.selected_shortcut = Some(shortcut.clone());
    app.image_selected_state.grid_id = None;

    // We must have a user to get to this action
    if let Some(auth_key) = app.settings.steamgrid_db.auth_key.clone() {
        // Create channel to communicate results
        let (tx, rx) = watch::channel(FetchStatus::Fetching);
        app.image_selected_state.grid_id_search = rx;

        let app_id = shortcut.app_id();
        let app_name = shortcut.name().to_string();

        // Spawn the search in the background instead of blocking the UI
        app.rt.spawn(async move {
            let client = steamgriddb_api::Client::new(auth_key);
            let search = CachedSearch::new(&client);
            let result = search.search(app_id, &app_name).await;

            let search_result = match result {
                Ok(grid_id) => Ok(grid_id),
                Err(e) => Err(e.to_string()),
            };

            let _ = tx.send(FetchStatus::Fetched(search_result));
        });
    }
}

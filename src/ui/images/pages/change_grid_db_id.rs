use tokio::sync::watch;
use tracing::{debug, warn};

use crate::{
    steamgriddb::CachedSearch,
    ui::{
        images::{image_select_state::ImageSelectState, useraction::UserAction},
        FetchStatus, MyEguiApp,
    },
};

pub fn render_page_change_grid_db_id(
    possible_names: &Vec<steamgriddb_api::search::SearchResult>,
    ui: &mut egui::Ui,
    state: &ImageSelectState,
) -> Option<UserAction> {
    let mut grid_id_text = state.grid_id.map(|id| id.to_string()).unwrap_or_default();
    ui.label("SteamGridDB ID")
        .on_hover_text("You can change this id to one you have found at the steamgriddb webpage");
    if ui.text_edit_singleline(&mut grid_id_text).changed() {
        if let Ok(grid_id) = grid_id_text.parse::<usize>() {
            return Some(UserAction::GridIdChanged(grid_id));
        }
    };

    for possible in possible_names {
        if ui.button(&possible.name).clicked() {
            return Some(UserAction::GridIdChanged(possible.id));
        }
    }

    ui.separator();
    if ui
        .button("Clear all images")
        .on_hover_text("Clicking this deletes all images for this shortcut")
        .clicked()
    {
        return Some(UserAction::ClearImages);
    }
    None
}

pub fn handle_grid_change(app: &mut MyEguiApp, grid_id: usize) {
    app.image_selected_state.grid_id = Some(grid_id);
    app.image_selected_state.possible_names = None;
    if let Some(auth_key) = &app.settings.steamgrid_db.auth_key {
        let client = steamgriddb_api::Client::new(auth_key);
        let mut cache = CachedSearch::new(&client);
        if let Some(shortcut) = &app.image_selected_state.selected_shortcut {
            cache.set_cache(shortcut.app_id(), shortcut.name(), grid_id);
            cache.save();
        }
    }
}


pub fn handle_correct_grid_request(app: &mut MyEguiApp) {
    let app_name = app
        .image_selected_state
        .selected_shortcut
        .as_ref()
        .map(|s| s.name().to_string())
        .unwrap_or_default();

    if let Some(auth_key) = app.settings.steamgrid_db.auth_key.clone() {
        debug!(app_name = %app_name, "Spawning async name search for grid ID correction");

        // Create channel to communicate results
        let (tx, rx) = watch::channel(FetchStatus::Fetching);
        app.image_selected_state.name_search = rx;

        // Clear any existing possible_names so we show loading state
        app.image_selected_state.possible_names = None;

        // Spawn the search in the background
        app.rt.spawn(async move {
            let client = steamgriddb_api::Client::new(auth_key);
            let search_results = client.search(&app_name).await;
            let results = search_results.unwrap_or_default();
            let _ = tx.send(FetchStatus::Fetched(results));
        });
    } else {
        warn!("No SteamGridDB auth key configured");
    }
}

use crate::ui::images::{image_select_state::ImageSelectState, useraction::UserAction};


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
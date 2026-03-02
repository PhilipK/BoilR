use crate::ui::images::{
    gametype::GameType, image_select_state::ImageSelectState, useraction::UserAction,
};

pub fn render_page_steam_images_overview(
    ui: &mut egui::Ui,
    state: &ImageSelectState,
) -> Option<UserAction> {
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

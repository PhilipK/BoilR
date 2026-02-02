use steam_shortcuts_util::shortcut::ShortcutOwned;

use crate::{steam::SteamUsersInfo, steamgriddb::ImageType, ui::FetchStatus};

use super::{ gamemode::GameMode, possible_image::PossibleImage,  gametype::GameType};

use tokio::sync::watch::{self, Receiver};


/// Result type for grid ID search operations
pub type GridIdSearchResult = Result<Option<usize>, String>;

pub struct ImageSelectState {
    pub selected_shortcut: Option<GameType>,
    pub grid_id: Option<usize>,

    pub steam_user: Option<SteamUsersInfo>,
    pub settings_error: Option<String>,
    pub steam_users: Option<Vec<SteamUsersInfo>>,
    pub user_shortcuts: Option<Vec<ShortcutOwned>>,
    pub game_mode: GameMode,
    pub image_type_selected: Option<ImageType>,
    pub image_options: Receiver<FetchStatus<Vec<PossibleImage>>>,
    pub steam_games: Option<Vec<crate::steam::SteamGameInfo>>,

    pub possible_names: Option<Vec<steamgriddb_api::search::SearchResult>>,

    /// Receiver for async grid ID search results
    pub grid_id_search: Receiver<FetchStatus<GridIdSearchResult>>,

    /// Receiver for async name search results (for "correct grid ID" feature)
    pub name_search: Receiver<FetchStatus<Vec<steamgriddb_api::search::SearchResult>>>,
}



impl Default for ImageSelectState {
    fn default() -> Self {
        Self {
            selected_shortcut: Default::default(),
            grid_id: Default::default(),
            steam_user: Default::default(),
            steam_users: Default::default(),
            settings_error: Default::default(),
            user_shortcuts: Default::default(),
            game_mode: GameMode::Shortcuts,
            image_type_selected: Default::default(),
            possible_names: None,
            image_options: watch::channel(FetchStatus::NeedsFetched).1,
            steam_games: None,
            grid_id_search: watch::channel(FetchStatus::NeedsFetched).1,
            name_search: watch::channel(FetchStatus::NeedsFetched).1,
        }
    }
}

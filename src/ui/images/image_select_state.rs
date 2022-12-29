use std::sync::Arc;

use steam_shortcuts_util::shortcut::ShortcutOwned;

use dashmap::DashMap;
use crate::{steam::SteamUsersInfo, steamgriddb::ImageType, ui::FetcStatus};

use super::{ gamemode::GameMode, possible_image::PossibleImage, texturestate::TextureDownloadState, gametype::GameType};

use tokio::sync::watch::{self, Receiver};
pub struct ImageSelectState {
    pub selected_shortcut: Option<GameType>,
    pub grid_id: Option<usize>,

    pub steam_user: Option<SteamUsersInfo>,
    pub settings_error: Option<String>,
    pub steam_users: Option<Vec<SteamUsersInfo>>,
    pub user_shortcuts: Option<Vec<ShortcutOwned>>,
    pub game_mode: GameMode,
    pub image_type_selected: Option<ImageType>,
    pub image_options: Receiver<FetcStatus<Vec<PossibleImage>>>,
    pub steam_games: Option<Vec<crate::steam::SteamGameInfo>>,
    pub image_handles: std::sync::Arc<DashMap<String, TextureDownloadState>>,

    pub possible_names: Option<Vec<steamgriddb_api::search::SearchResult>>,
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
            image_options: watch::channel(FetcStatus::NeedsFetched).1,
            image_handles: Arc::new(DashMap::new()),
            steam_games: None,
        }
    }
}


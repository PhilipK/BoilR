
use steam_shortcuts_util::shortcut::ShortcutOwned;

use crate::{steam::SteamGameInfo};


#[derive(Debug, Clone)]
pub enum GameType {
    Shortcut(Box<ShortcutOwned>),
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

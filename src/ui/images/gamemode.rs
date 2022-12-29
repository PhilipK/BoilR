
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameMode {
    Shortcuts,
    SteamGames,
}

impl GameMode {
    pub fn is_shortcuts(&self) -> bool {
        match self {
            GameMode::Shortcuts => true,
            GameMode::SteamGames => false,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            GameMode::Shortcuts => "Images for shortcuts",
            GameMode::SteamGames => "Images for steam games",
        }
    }
}
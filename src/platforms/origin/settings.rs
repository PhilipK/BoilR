use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]

pub struct EASettings {
    pub enabled: bool,
    /// Windows: close an open EA app before each game, so the game's shortcut starts it
    /// inside Steam and Steam's overlay (and Big Picture's Resume) reaches the game. Without
    /// it, the overlay only reaches games started while the EA app is closed; an open EA app
    /// takes the game over, outside Steam. Off by default: closing the EA app stops its
    /// downloads, and another EA game running at the time loses its launcher.
    #[serde(default)]
    pub steam_overlay: bool,
}

impl Default for EASettings {
    fn default() -> Self {
        Self {
            enabled: true,
            steam_overlay: false,
        }
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct UbisoftSettings {
    pub enabled: bool,
    /// Windows: close an open Ubisoft Connect before each game, so the game's shortcut starts
    /// it inside Steam and Steam's overlay (and Big Picture's Resume) reaches the game.
    /// Without it, the overlay only reaches games started while Ubisoft Connect is closed; an
    /// open Ubisoft Connect takes the game over, outside Steam. Off by default: an open
    /// Ubisoft Connect can only be ended (it hides itself when asked to close), which stops
    /// its downloads and leaves another Ubisoft game running at the time without its
    /// launcher.
    ///
    /// Either way, a Ubisoft Connect that a game's shortcut started is asked to quit 5
    /// seconds after the game (its `closebehavior` is set to quit for that while), and ended
    /// if it doesn't. Its quit doesn't wait for cloud-save uploads; a cut-off upload is
    /// probably retried when that game next starts.
    #[serde(default)]
    pub steam_overlay: bool,
}

impl Default for UbisoftSettings {
    fn default() -> Self {
        #[cfg(target_family = "unix")]
        let enabled = false;
        #[cfg(target_family = "windows")]
        let enabled = true;
        Self {
            enabled,
            steam_overlay: false,
        }
    }
}

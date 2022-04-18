use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EpicGamesLauncherSettings {
    pub enabled: bool,
    pub location: Option<String>,

    #[cfg(target_family = "unix")]
    pub create_symlinks: bool,

    pub safe_launch : Vec<String>
}

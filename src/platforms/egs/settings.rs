use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EpicGamesLauncherSettings {
    pub enabled: bool,
    pub safe_launch: Vec<String>,
}

impl Default for EpicGamesLauncherSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            safe_launch: Default::default(),
        }
    }
}

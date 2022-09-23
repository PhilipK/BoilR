use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EpicGamesLauncherSettings {
    pub enabled: bool,
    pub safe_launch: Vec<String>,
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Default,Deserialize, Clone)]
pub struct EpicGamesLauncherSettings {
    pub enabled: bool,
    pub safe_launch: Vec<String>,
}

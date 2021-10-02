use serde::{Serialize,Deserialize};

#[derive(Debug,Serialize, Deserialize,Clone)]
pub struct EpicGamesLauncherSettings {
    pub enabled: bool,
    pub location: Option<String>,
}



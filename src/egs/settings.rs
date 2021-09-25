use serde::Deserialize;

#[derive(Debug, Deserialize,Clone)]
pub struct EpicGamesLauncherSettings {
    pub enabled: bool,
    pub location: Option<String>,
}



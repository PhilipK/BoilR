use std::env;
use config::{ConfigError, Config, File, Environment};


#[derive(Debug, Deserialize)]
pub struct Settings {
    debug: bool,
    database: EpicGamesLauncher,
}

//https://github.com/JosefNemec/Playnite/tree/master/source/Plugins/OriginLibrary
//https://github.com/JosefNemec/Playnite/blob/master/source/Plugins/OriginLibrary/Origin.cs#L109
impl Settings {

    pub fn new() -> Result<Self, ConfigError> {

    }
}
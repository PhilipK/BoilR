use crate::{egs::EpicGamesLauncherSettings, legendary::LegendarySettings, steam::SteamSettings, steamgriddb::SteamGridDbSettings};

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::{env};

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub debug: bool,
    pub epic_games: EpicGamesLauncherSettings,
    pub legendary: LegendarySettings,
    pub steamgrid_db: SteamGridDbSettings,
    pub steam: SteamSettings
}

//https://github.com/JosefNemec/Playnite/tree/master/source/Plugins/OriginLibrary
//https://github.com/JosefNemec/Playnite/blob/master/source/Plugins/OriginLibrary/Origin.cs#L109
impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = Config::new();

        let default_str = include_str!("defaultconfig.toml");
        s.merge(File::from_str(default_str, config::FileFormat::Toml))?;
        
        let enable_legendary = true;
        let enable_epic = false;

        #[cfg(target_os = "windows")]
        let enable_legendary = false;
        #[cfg(target_os = "windows")]
        let enable_epic = true;

        s.set_default("legendary.enabled", enable_legendary)?;
        s.set_default("epic_games.enabled", enable_epic)?;
        

        // Start off by merging in the "default" configuration file
        s.merge(File::with_name("config.toml").required(false))?;

        // Add in the current environment file
        // Default to 'development' env
        // Note that this file is _optional_
        let env = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());
        s.merge(File::with_name(&format!("config/{}", env)).required(false))?;

        // Add in a local configuration file
        // This file shouldn't be checked in to git
        s.merge(File::with_name("local.toml").required(false))?;

        // Add in settings from the environment (with a prefix of STEAMSYNC)
        // Eg.. `STEAMSYNC_DEBUG=1 ./target/app` would set the `debug` key
        s.merge(Environment::with_prefix("steamsync").separator("-"))?;

        s.try_into()
    }
}

use crate::{
    egs::EpicGamesLauncherSettings, legendary::LegendarySettings, steamgriddb::SteamGridDbSettings,
};

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::{env, path::Path};

#[derive(Debug, Deserialize, Default)]
pub struct Settings {
    pub debug: bool,
    pub epic_games: EpicGamesLauncherSettings,
    pub legendary: LegendarySettings,
    pub steamgrid_db: SteamGridDbSettings,
}

#[derive(Debug, Deserialize, Default)]
struct SettingsOptional {
    pub debug: Option<bool>,
    pub epic_games: Option<EpicGamesLauncherSettings>,
    pub legendary: Option<LegendarySettings>,
    pub steamgrid_db: Option<SteamGridDbSettings>,
}

impl From<SettingsOptional> for Settings {
    fn from(optional: SettingsOptional) -> Self {
        Self {
            debug: optional.debug.unwrap_or(false),
            epic_games: optional.epic_games.unwrap_or(Default::default()),
            legendary: optional.legendary.unwrap_or(Default::default()),
            steamgrid_db: optional.steamgrid_db.unwrap_or(Default::default()),
        }
    }
}

//https://github.com/JosefNemec/Playnite/tree/master/source/Plugins/OriginLibrary
//https://github.com/JosefNemec/Playnite/blob/master/source/Plugins/OriginLibrary/Origin.cs#L109
impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let default = Self::default();
        if !Path::new("config.toml").exists() {
            return Ok(default);
        }
        let mut s = Config::new();

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

        // Add in settings from the environment (with a prefix of STEAM_SYNC)
        // Eg.. `STEAM_SYNC_DEBUG=1 ./target/app` would set the `debug` key
        s.merge(Environment::with_prefix("steam_sync"))?;

        // You can deserialize (and thus freeze) the entire configuration as
        let optionals: SettingsOptional = s.try_into()?;

        Ok(optionals.into())
    }
}

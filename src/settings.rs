use crate::{
    egs::EpicGamesLauncherSettings, gog::GogSettings, itch::ItchSettings,
    legendary::LegendarySettings, lutris::settings::LutrisSettings, origin::OriginSettings,
    steam::SteamSettings, steamgriddb::SteamGridDbSettings, uplay::UplaySettings, heroic::HeroicSettings, amazon::AmazonSettings,
};

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    pub debug: bool,
    pub blacklisted_games: Vec<u32>,
    pub epic_games: EpicGamesLauncherSettings,
    pub legendary: LegendarySettings,
    pub itch: ItchSettings,
    pub steamgrid_db: SteamGridDbSettings,
    pub steam: SteamSettings,
    pub origin: OriginSettings,
    pub gog: GogSettings,
    pub uplay: UplaySettings,
    pub lutris: LutrisSettings,
    pub heroic: HeroicSettings,
    pub amazon: AmazonSettings,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let mut s = Config::new();

        let default_str = include_str!("defaultconfig.toml");
        s.merge(File::from_str(default_str, config::FileFormat::Toml))?;

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

        let mut result: Result<Self, ConfigError> = s.try_into();

        sanitize_auth_key(&mut result);

        result
    }
}

fn sanitize_auth_key(result: &mut Result<Settings, ConfigError>) {
    if let Ok(result) = result.as_mut() {
        if let Some(auth_key) = result.steamgrid_db.auth_key.as_ref() {
            if auth_key == "Write your authentication key between these quotes" {
                result.steamgrid_db.auth_key = None;
            }
        }
    }
}

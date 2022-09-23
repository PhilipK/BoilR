use crate::{
    amazon::AmazonSettings, config::get_config_file, flatpak::FlatpakSettings, gog::GogSettings,
    itch::ItchSettings, legendary::LegendarySettings, lutris::settings::LutrisSettings,
    origin::OriginSettings, platforms::EpicGamesLauncherSettings, steam::SteamSettings,
    steamgriddb::SteamGridDbSettings, uplay::UplaySettings,
};

#[cfg(target_family = "unix")]
use crate::heroic::HeroicSettings;

use crate::bottles::BottlesSettings;

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    pub debug: bool,
    pub config_version: Option<usize>,
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
    #[cfg(target_family = "unix")]
    pub heroic: HeroicSettings,
    pub amazon: AmazonSettings,
    pub flatpak: FlatpakSettings,
    pub bottles: BottlesSettings,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let default_str = include_str!("defaultconfig.toml");
        let config_file = get_config_file();
        let config_file = config_file.to_string_lossy();
        let env = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let config = Config::builder()
            .add_source(File::from_str(default_str, config::FileFormat::Toml))
            // Start off by merging in the "default" configuration file
            .add_source(File::with_name(config_file.as_ref()).required(false))
            // Add in the current environment file
            // Default to 'development' env
            // Note that this file is _optional_
            .add_source(File::with_name(&format!("config/{}", env)).required(false))
            // Add in a local configuration file
            // This file shouldn't be checked in to git
            .add_source(File::with_name("local.toml").required(false))
            // Add in settings from the environment (with a prefix of STEAMSYNC)
            // Eg.. `STEAMSYNC_DEBUG=1 ./target/app` would set the `debug` key
            .add_source(Environment::with_prefix("boilr").separator("-"))
            .build()?;
        let mut settings = config.try_deserialize::<Settings>()?;
        sanitize_auth_key(&mut settings);
        Ok(settings)
    }
}

fn sanitize_auth_key(result: &mut Settings) {
    if let Some(auth_key) = result.steamgrid_db.auth_key.as_ref() {
        if auth_key == "Write your authentication key between these quotes" {
            result.steamgrid_db.auth_key = None;
        }
    }
}

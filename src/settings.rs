use crate::{
    config::get_config_file,
    platforms::{AmazonSettings, GogSettings, HeroicSettings, OriginSettings},
    platforms::{
        EpicGamesLauncherSettings, FlatpakSettings, ItchSettings, LegendarySettings,
        LutrisSettings, UplaySettings,
    },
    steam::SteamSettings,
    steamgriddb::SteamGridDbSettings,
};

use crate::platforms::BottlesSettings;

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env};

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

pub fn load_setting_sections() -> eyre::Result<HashMap<String, String>> {
    let config_file_path = get_config_file();
    let content = std::fs::read_to_string(config_file_path)?;
    let mut result = HashMap::new();
    let lines = content.lines();
    let mut current_section_lines: Vec<String> = vec![];
    let mut current_section_name: Option<String> = Option::None;
    for line in lines {
        if line.starts_with("[") && line.ends_with("]") {
            add_sections(&current_section_name, &current_section_lines, &mut result);
            current_section_name = Some(line[1..line.len() - 1].to_string());
        } else {
            current_section_lines.push(line.to_string());
        }
    }
    add_sections(&current_section_name, &current_section_lines, &mut result);
    Ok(result)
}

fn add_sections(current_section_name: &Option<String>, current_section_lines: &Vec<String>, result: &mut HashMap<String, String>) {
    if let Some(old_section_name) = current_section_name {
        let mut section_string = String::new();
        for line in current_section_lines {
            section_string.push_str(line);
            section_string.push('\n');
        }
        result.insert(old_section_name.to_string(), section_string);
    }
}

fn sanitize_auth_key(result: &mut Settings) {
    if let Some(auth_key) = result.steamgrid_db.auth_key.as_ref() {
        if auth_key == "Write your authentication key between these quotes" {
            result.steamgrid_db.auth_key = None;
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    pub fn load_sections() {
        let sections = load_setting_sections();

        dbg!(&sections);
        assert!(sections.is_ok())
    }
}

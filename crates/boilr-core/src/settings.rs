use crate::{
    config::get_config_file, owned::OwnedGamesSettings, steam::SteamSettings,
    steamgriddb::SteamGridDbSettings,
};

use config::{Config, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    pub debug: bool,
    pub config_version: Option<usize>,
    pub blacklisted_games: Vec<u32>,
    pub steamgrid_db: SteamGridDbSettings,
    pub steam: SteamSettings,
    /// How games a store account owns but that are not installed show up in Steam.
    #[serde(default)]
    pub owned_games: OwnedGamesSettings,
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
            .add_source(File::with_name(&format!("config/{env}")).required(false))
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
    let content = std::fs::read_to_string(get_config_file())?;
    Ok(platform_sections_of(&content))
}

/// The platform sections of a `config.toml`, by name: every `[section]` except those of
/// [`Settings`] itself.
fn platform_sections_of(content: &str) -> HashMap<String, String> {
    let mut result = HashMap::new();
    let mut current_section_lines: Vec<String> = vec![];
    let mut current_section_name: Option<String> = Option::None;
    for line in content.lines() {
        if line.starts_with('[') && line.ends_with(']') {
            add_sections(&current_section_name, &current_section_lines, &mut result);
            current_section_name = line.get(1..line.len() - 1).map(|s| s.to_string());
            current_section_lines.clear();
        } else {
            current_section_lines.push(line.to_string());
        }
    }
    add_sections(&current_section_name, &current_section_lines, &mut result);

    // Sections of `Settings` itself, not of a platform.
    let blacklisted_sections = ["steamgrid_db", "steam", "owned_games"];
    for section in blacklisted_sections {
        let _ = result.remove(section);
    }
    result
}

/// Saves the settings followed by one `[code_name]` section per platform, given as
/// `(code_name, serialized settings)` pairs.
pub fn save_settings(
    settings: &Settings,
    platform_sections: &[(String, String)],
) -> eyre::Result<()> {
    let toml = settings_toml(settings, platform_sections)?;
    let config_path = crate::config::get_config_file();
    if let Some(folder) = config_path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(config_path, toml)?;
    Ok(())
}

/// The text of `config.toml`: the settings, then the platform sections, one empty line
/// between sections. A section read back keeps the empty line that ended it, so it is
/// trimmed: otherwise every save would add one.
fn settings_toml(
    settings: &Settings,
    platform_sections: &[(String, String)],
) -> eyre::Result<String> {
    let mut toml = toml::to_string(&settings)?;
    for (code_name, platform_string) in platform_sections {
        let toml_end = toml.trim_end().len();
        toml.truncate(toml_end);
        toml.push_str("\n\n[");
        toml.push_str(code_name);
        toml.push_str("]\n");
        let body = platform_string.trim();
        if !body.is_empty() {
            toml.push_str(body);
            toml.push('\n');
        }
    }
    Ok(toml)
}

fn add_sections(
    current_section_name: &Option<String>,
    current_section_lines: &Vec<String>,
    result: &mut HashMap<String, String>,
) {
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

    fn default_settings() -> eyre::Result<Settings> {
        let config = Config::builder()
            .add_source(File::from_str(
                include_str!("defaultconfig.toml"),
                config::FileFormat::Toml,
            ))
            .build()?;
        Ok(config.try_deserialize::<Settings>()?)
    }

    #[test]
    fn saving_again_and_again_adds_no_empty_lines() -> eyre::Result<()> {
        let settings = default_settings()?;
        let sections = vec![
            (
                "epic_account".to_string(),
                "enabled = true\n\n\n".to_string(),
            ),
            ("gog".to_string(), "enabled = false\n".to_string()),
        ];
        let first = settings_toml(&settings, &sections)?;
        let mut read_back: Vec<(String, String)> =
            platform_sections_of(&first).into_iter().collect();
        read_back.sort();
        let second = settings_toml(&settings, &read_back)?;
        assert_eq!(first, second);
        assert!(!first.contains("\n\n\n"), "{first}");
        Ok(())
    }

    #[test]
    fn owned_games_settings_are_not_a_platform() -> eyre::Result<()> {
        let settings = default_settings()?;
        assert!(settings.owned_games.skip_games_in_steam);
        assert!(settings.steamgrid_db.match_by_store_id);
        let toml = settings_toml(
            &settings,
            &[("gog".to_string(), "enabled = true".to_string())],
        )?;
        assert!(toml.contains("[owned_games]"), "{toml}");
        let sections = platform_sections_of(&toml);
        assert_eq!(sections.keys().collect::<Vec<_>>(), vec!["gog"]);
        Ok(())
    }
}

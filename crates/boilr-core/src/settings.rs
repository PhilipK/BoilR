use crate::{config::get_config_file, steam::SteamSettings, steamgriddb::SteamGridDbSettings};

use config::{Config, ConfigError, Environment, File, Map};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Settings {
    pub debug: bool,
    pub config_version: Option<usize>,
    pub blacklisted_games: Vec<u32>,
    pub steamgrid_db: SteamGridDbSettings,
    pub steam: SteamSettings,
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
            .add_source(legacy_env_overrides(None))
            .add_source(env_overrides(None))
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
        if line.starts_with('[') && line.ends_with(']') {
            add_sections(&current_section_name, &current_section_lines, &mut result);
            current_section_name = line.get(1..line.len() - 1).map(|s| s.to_string());
            current_section_lines.clear();
        } else {
            current_section_lines.push(line.to_string());
        }
    }
    add_sections(&current_section_name, &current_section_lines, &mut result);

    let blacklisted_sections = ["steamgrid_db", "steam"];
    for section in blacklisted_sections {
        let _ = result.remove(section);
    }
    Ok(result)
}

/// Saves the settings followed by one `[code_name]` section per platform, given as
/// `(code_name, serialized settings)` pairs.
pub fn save_settings(
    settings: &Settings,
    platform_sections: &[(String, String)],
) -> eyre::Result<()> {
    let mut toml = toml::to_string(&settings)?;

    for (code_name, platform_string) in platform_sections {
        let section_name = format!("[{code_name}]");
        toml.push('\n');
        toml.push_str(section_name.as_str());
        toml.push('\n');
        toml.push_str(platform_string.as_str());
    }

    let config_path = crate::config::get_config_file();
    if let Some(folder) = config_path.parent() {
        std::fs::create_dir_all(folder)?;
    }
    std::fs::write(config_path, toml)?;
    Ok(())
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

/// Environment overrides, e.g. `BOILR_STEAMGRID_DB__AUTH_KEY=... boilr --no-ui` sets
/// `steamgrid_db.auth_key`. `__` separates sections because keys contain `_`.
fn env_overrides(source: Option<Map<String, String>>) -> Environment {
    Environment::with_prefix("boilr")
        .prefix_separator("_")
        .separator("__")
        .source(source)
}

/// The old `boilr-steamgrid_db-auth_key` form. Shells can't set names containing `-` (#367),
/// but `env` can, so it keeps working.
fn legacy_env_overrides(source: Option<Map<String, String>>) -> Environment {
    Environment::with_prefix("boilr")
        .separator("-")
        .source(source)
}

fn sanitize_auth_key(result: &mut Settings) {
    if let Some(auth_key) = result.steamgrid_db.auth_key.as_ref() {
        if auth_key == "Write your authentication key between these quotes" {
            result.steamgrid_db.auth_key = None;
        }
    }
}

#[cfg(test)]
mod env_tests {
    use super::*;

    fn auth_key_from(env: &[(&str, &str)]) -> Option<String> {
        let source: Map<String, String> = env
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        let config = Config::builder()
            .add_source(legacy_env_overrides(Some(source.clone())))
            .add_source(env_overrides(Some(source)))
            .build()
            .ok()?;
        config.get_string("steamgrid_db.auth_key").ok()
    }

    #[test]
    fn shell_friendly_env_var_sets_auth_key() {
        let key = auth_key_from(&[("BOILR_STEAMGRID_DB__AUTH_KEY", "abc")]);
        assert_eq!(key.as_deref(), Some("abc"));
    }

    #[test]
    fn legacy_dash_env_var_still_sets_auth_key() {
        let key = auth_key_from(&[("boilr-steamgrid_db-auth_key", "abc")]);
        assert_eq!(key.as_deref(), Some("abc"));
    }
}

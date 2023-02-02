use std::path::Path;

use crate::{platforms::get_platforms, settings::save_settings};

pub fn migrate_config() {
    let version = &crate::settings::Settings::new()
        .map(|s| s.config_version)
        .unwrap_or_default();

    let mut save_version = false;
    if version.is_none() {
        //Migration from 0 to 1
        let old_path = &Path::new("config.toml");
        if old_path.exists() {
            println!("Migrating from configuration version 0 to version 1");
            let new_path = crate::config::get_config_file();
            println!("Your configuration file will be moved to {new_path:?}");
            let _ = std::fs::copy(old_path, new_path);
            let _ = std::fs::remove_file(old_path);
        }

        let old_path = &Path::new(".thumbnails");
        if old_path.exists() {
            //thumbnails are just cache can be removed
            let _ = std::fs::remove_dir_all(old_path);
        }

        let old_path = &Path::new("cache.json");
        if old_path.exists() {
            let new_path = crate::config::get_cache_file();
            let _ = std::fs::copy(old_path, new_path);
            let _ = std::fs::remove_file(old_path);
        }
        save_version = true;
    }

    if save_version {
        if let Ok(mut settings) = crate::settings::Settings::new() {
            settings.config_version = Some(1);
            let platforms = get_platforms();
            if let Err(err) = save_settings(&settings, &platforms){
                eprintln!("Failed to load settings {err:?}");
            }
        }
    }
}

mod amazon;
mod config;
mod egs;
mod gog;
mod heroic;
mod itch;
mod legendary;
mod lutris;
mod origin;
mod platform;
mod settings;
mod steam;
mod steamgriddb;
mod sync;
mod ui;
mod uplay;
use std::{error::Error, path::Path};

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args();
    migrate_config();
    if args.len() > 1 && args.nth(1).unwrap_or_default() == "--no-ui" {
        ui::run_sync();
        Ok(())
    } else {
        ui::run_ui()
    }
}

fn migrate_config() {
    let version = &settings::Settings::new()
        .map(|s| s.config_version)
        .unwrap_or_default();

    let mut save_version = false;
    if version.is_none() {
        //Migration from 0 to 1
        let old_path = &Path::new("config.toml");
        if old_path.exists() {
            println!("Migrating from configuration version 0 to version 1");
            let new_path = config::get_config_file();
            println!("Your configuration file will be moved to {:?}", new_path);
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
            let new_path = config::get_thumbnails_folder();
            let _ = std::fs::copy(old_path, new_path);
            let _ = std::fs::remove_file(old_path);
        }
        save_version = true;
    }

    if save_version {
        if let Ok(mut settings) = settings::Settings::new() {
            settings.config_version = Some(1);
            ui::MyEguiApp::save_settings_to_file(&settings);
        }
    }
}

mod amazon;
mod config;
mod egs;
mod gog;
mod heroic;
mod itch;
mod legendary;
mod lutris;
mod migration;
mod origin;
mod platform;
mod settings;
mod steam;
mod steamgriddb;
mod sync;
mod ui;
mod uplay;
mod flatpak;
use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    ensure_config_folder();
    migration::migrate_config();

    let mut args = std::env::args();
    if args.len() > 1 && args.nth(1).unwrap_or_default() == "--no-ui" {
        ui::run_sync();
        Ok(())
    } else {
        ui::run_ui()
    }
}

fn ensure_config_folder() {
    let path = config::get_config_folder();
    let _ = std::fs::create_dir_all(&path);
}

#![deny(clippy::unwrap_in_result)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::unwrap_used)]

mod config;
mod migration;
mod platforms;
mod settings;
mod steam;
mod steamgriddb;
mod sync;
mod ui;

use color_eyre::eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;
    ensure_config_folder();
    migration::migrate_config();

    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--no-ui".to_string()) {
        ui::run_sync();
    } else {
        ui::run_ui(args);
    }
    Ok(())
}

fn ensure_config_folder() {
    let path = config::get_config_folder();
    let _ = std::fs::create_dir_all(path);
}

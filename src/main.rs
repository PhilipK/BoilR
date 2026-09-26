#![deny(clippy::unwrap_in_result)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]

mod platforms;
mod single_instance;
mod ui;

// Core modules live in the boilr-core crate; importing them here keeps `crate::settings` etc.
// paths working throughout the UI and platform code.
use boilr_core::{config, migration, settings, steam, steamgriddb, sync};

use color_eyre::eyre::Result;

fn main() -> Result<()> {
    color_eyre::install()?;
    ensure_config_folder();

    // Acquire single instance lock
    let _instance_lock = match single_instance::InstanceLock::acquire() {
        Ok(lock) => lock,
        Err(msg) => {
            eprintln!("Error: {}", msg);
            eprintln!("Please close the other instance of BoilR first.");
            return Ok(());
        }
    };

    migration::migrate_config(|| platforms::platform_sections(&platforms::get_platforms()));

    let args: Vec<String> = std::env::args().collect();
    if args.contains(&"--no-ui".to_string()) {
        ui::run_sync()?;
    } else {
        ui::run_ui(args)?;
    }
    Ok(())
}

fn ensure_config_folder() {
    let path = config::get_config_folder();
    let _ = std::fs::create_dir_all(path);
}

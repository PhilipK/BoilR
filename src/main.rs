#![deny(clippy::unwrap_in_result)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]

mod single_instance;

use boilr::{platforms, ui};
use boilr_core::{config, launch_and_wait, migration};

use color_eyre::eyre::Result;

fn main() -> Result<()> {
    // A Steam shortcut that waits for its game: no interface, no instance lock.
    let launch_args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(result) = launch_and_wait::run_if_asked(&launch_args) {
        return result;
    }
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

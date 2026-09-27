#![deny(clippy::unwrap_in_result)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]

mod logging;
mod single_instance;

use boilr::{platforms, ui};
use boilr_core::{config, migration};

use color_eyre::eyre::Result;
use tracing::{error, info};

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

    // Keep the guard alive for the entire program so file logs are flushed.
    let _log_guard = logging::init_logging();
    info!("BoilR starting up");

    migration::migrate_config(|| platforms::platform_sections(&platforms::get_platforms()));

    let args: Vec<String> = std::env::args().collect();
    let result = if args.contains(&"--no-ui".to_string()) {
        info!("Running in headless mode (--no-ui)");
        ui::run_sync()
    } else {
        info!("Running in GUI mode");
        ui::run_ui(args)
    };

    if let Err(ref e) = result {
        error!(error = %e, "BoilR encountered an error");
    }

    info!("BoilR shutting down");
    result
}

fn ensure_config_folder() {
    let path = config::get_config_folder();
    let _ = std::fs::create_dir_all(path);
}

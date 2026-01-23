#![deny(clippy::unwrap_in_result)]
#![deny(clippy::get_unwrap)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::indexing_slicing)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]
#![deny(clippy::todo)]

mod config;
mod logging;
mod migration;
mod platforms;
mod settings;
mod single_instance;
mod steam;
mod steamgriddb;
mod sync;
mod ui;

use color_eyre::eyre::Result;
use tracing::{error, info};

fn main() -> Result<()> {
    color_eyre::install()?;
    ensure_config_folder();

    // Initialize logging - keep the guard alive for the entire program
    let _log_guard = logging::init_logging();

    info!("BoilR starting up");

    // Acquire single instance lock
    let _instance_lock = match single_instance::InstanceLock::acquire() {
        Ok(lock) => lock,
        Err(msg) => {
            error!("{}", msg);
            eprintln!("Error: {}", msg);
            eprintln!("Please close the other instance of BoilR first.");
            return Ok(());
        }
    };

    migration::migrate_config();

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

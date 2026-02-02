use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::get_config_folder;

/// Returns the path to the log file
pub fn get_log_file_path() -> PathBuf {
    get_config_folder().join("boilr.log")
}

/// Initialize the logging system with both file and console output.
/// Returns a guard that must be kept alive for the duration of the program
/// to ensure all logs are flushed to the file.
pub fn init_logging() -> WorkerGuard {
    let log_folder = get_config_folder();
    let _ = std::fs::create_dir_all(&log_folder);

    // Create a file appender that writes to boilr.log
    let file_appender = tracing_appender::rolling::never(&log_folder, "boilr.log");
    let (non_blocking_file, guard) = tracing_appender::non_blocking(file_appender);

    // Set up the filter - default to INFO, can be overridden with RUST_LOG env var
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,boilr=debug"));

    // Build the subscriber with both file and stdout output
    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .with_writer(non_blocking_file)
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_file(true)
                .with_line_number(true),
        )
        .with(
            fmt::layer()
                .with_writer(std::io::stderr)
                .with_ansi(true)
                .with_target(true)
                .compact(),
        )
        .init();

    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "BoilR logging initialized"
    );
    tracing::info!(log_path = %get_log_file_path().display(), "Log file location");

    guard
}

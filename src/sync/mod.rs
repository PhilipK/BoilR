#[cfg(target_os = "linux")]
mod symlinks;
mod synchronization;

pub use synchronization::run_sync;

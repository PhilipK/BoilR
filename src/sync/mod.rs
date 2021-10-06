#[cfg(target_os = "linux")]
mod symlinks;
mod sync;

pub use sync::run_sync;
